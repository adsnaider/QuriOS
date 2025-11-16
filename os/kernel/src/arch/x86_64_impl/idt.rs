mod handlers;

use handlers::{Isr, IsrHandler, PanicHandler};
use pic8259::ChainedPics;
use qapi::caps::CapResult;
use qapi::caps::sync_ipc::ExceptionArgs;
use qapi::exception::ExceptionKind;
use qapi::syscall::SyscallArgs;
use sync::cell::AtomicLazyCell;
use tap::TapFallible;
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptDescriptorTable, PageFaultErrorCode};
use x86_64::{PrivilegeLevel, VirtAddr as VirtAddrImpl};

use super::X64Sys;
use super::exec::Interrupt;
use crate::arch::mem::{MemorySegment, VirtAddr, user_buffer_read_page_fault_call_gate};
use crate::arch::x86_64_impl::exec::{Exception, ExceptionCtx, IrqCtx};
use crate::arch::x86_64_impl::gdt;
use crate::core_local::core_cell::{BindError, GetError, Lock};
use crate::core_local::{CORE_LOCAL_SAFE_BUFFER_LOCK, CoreCell};
use crate::notify::Notification;
use crate::syscall::{ring3_exception_handler, syscall_handler};

const SYSCALL_INT: u8 = 0x80;

pub struct IrqCtrlTable(CoreCell<Lock<[Option<Notification<X64Sys>>; 15]>>);
pub(super) static IRQ_CTRL_TABLE: IrqCtrlTable = IrqCtrlTable::new();

impl IrqCtrlTable {
    pub const fn new() -> Self {
        Self(CoreCell::new(Lock::new([const { None }; 15])))
    }

    pub fn bind_to_core(&self) -> Result<(), BindError> {
        self.0.bind().map(|_| ())
    }

    pub fn set(&self, irq: u8, notification: Option<Notification<X64Sys>>) -> Result<(), GetError> {
        self.0.try_get()?.lock()[irq as usize] = notification;
        Ok(())
    }

    pub fn get(&self, irq: u8) -> Result<Option<Notification<X64Sys>>, GetError> {
        Ok(self.0.try_get()?.lock()[irq as usize].clone())
    }
}

/// Initializes the IDT and sets up the 8259 PIC.
pub fn init() {
    init_idt();
    log::info!("Interrupt tables initialized");
}

pub struct ForwardRing3Exceptions<const ID: usize>;
impl<const ID: usize> IsrHandler<Interrupt, ()> for ForwardRing3Exceptions<ID> {
    extern "sysv64" fn call(ctx: ExceptionCtx<Interrupt>) {
        match ctx.interrupt_stack_frame().code_segment.rpl() {
            PrivilegeLevel::Ring0 => panic!("Unexpected exception in kernel code: {ID}\n{ctx:#?}"),
            PrivilegeLevel::Ring1 | PrivilegeLevel::Ring2 => {
                unreachable!("Unexpected ring usage in dual mode processor use")
            }
            PrivilegeLevel::Ring3 => ring3_exception_handler(
                ExceptionArgs {
                    kind: ID,
                    code: u64::MAX,
                    extra: 0,
                },
                IrqCtx::Interrupt(ctx),
            ),
        }
    }
}
impl<const ID: usize> IsrHandler<Exception, ()> for ForwardRing3Exceptions<ID> {
    extern "sysv64" fn call(ctx: ExceptionCtx<Exception>) {
        match ctx.interrupt_stack_frame().code_segment.rpl() {
            PrivilegeLevel::Ring0 => panic!("Unexpected exception in kernel code: {ID}\n{ctx:#?}"),
            PrivilegeLevel::Ring1 | PrivilegeLevel::Ring2 => {
                unreachable!("Unexpected ring usage in dual mode processor use")
            }
            PrivilegeLevel::Ring3 => ring3_exception_handler(
                ExceptionArgs {
                    kind: ID,
                    code: ctx.error_code(),
                    extra: 0,
                },
                IrqCtx::Exception(ctx),
            ),
        }
    }
}
pub struct SyscallHandler;
impl IsrHandler<Interrupt, ()> for SyscallHandler {
    extern "sysv64" fn call(ctx: ExceptionCtx<Interrupt>) {
        match ctx.interrupt_stack_frame().code_segment.rpl() {
            PrivilegeLevel::Ring0 => panic!("Unexpected syscall within kernel code\n{ctx:#?}"),
            PrivilegeLevel::Ring1 | PrivilegeLevel::Ring2 => {
                unreachable!("Unexpected ring usage in dual mode processor use")
            }
            PrivilegeLevel::Ring3 => {
                let regs = ctx.scratch_regs();
                let args = SyscallArgs::new_raw(
                    regs.rdi as usize,
                    [
                        regs.rsi as usize,
                        regs.rdx as usize,
                        regs.rcx as usize,
                        regs.r8 as usize,
                        regs.r9 as usize,
                    ],
                );
                let res = syscall_handler(args, IrqCtx::Interrupt(ctx));
                log::debug!("Syscall response: {res:?}");
                // SAFETY: Done handling syscall so we have exclusive asccess to the interrupt stack frame
                unsafe { ctx.scratch_regs_mut().rax = res.into_isize() as u64 };
            }
        }
    }
}

pub struct IrqHandler<const IRQ: u8>;
impl<const IRQ: u8> IsrHandler<Interrupt, ()> for IrqHandler<IRQ> {
    extern "sysv64" fn call(ctx: ExceptionCtx<Interrupt>) {
        let mut dispatcher = None;
        log::trace!("IRQ - {IRQ}");
        // IRQ Ctrl table is bound to a single core
        if let Ok(pics) = PICS.try_get() {
            let mut pics = pics.lock();
            log::trace!("{IRQ} - Notifying end of interrupt");
            // SAFETY: The interrupt we are notifying is the one that has been triggered
            unsafe { pics.notify_end_of_interrupt(IRQ + PIC_OFFSET) };
            let handler = IRQ_CTRL_TABLE
                .get(IRQ)
                .expect("PIC and IRQ ctrl not bound to the same thread");
            if let Some(notification) = handler {
                log::trace!("Found IRQ handler");
                if let Ok(d) = notification
                    .signal(IrqCtx::Interrupt(ctx))
                    .tap_err(|e| log::warn!("Unable to notify IRQ handler: {IRQ} - {e}"))
                {
                    dispatcher = d;
                }
            }
        }
        if let Some(dispatcher) = dispatcher {
            log::trace!("Notifying IRQ ({IRQ}) handler");
            dispatcher.dispatch();
        }
    }
}

const PIC_OFFSET: u8 = 32;

// SAFETY: The offset provided avoids colliding with other exceptions and interrupts.
static PICS: CoreCell<Lock<ChainedPics>> = CoreCell::new(Lock::new(unsafe {
    ChainedPics::new_contiguous(PIC_OFFSET)
}));

/// Initializes the interrupt descriptor table.
fn init_idt() {
    static IDT: AtomicLazyCell<InterruptDescriptorTable> = AtomicLazyCell::new(|| {
        let mut idt = InterruptDescriptorTable::new();
        // Exceptions.
        idt.breakpoint
            .register(ForwardRing3Exceptions::<{ ExceptionKind::BREAKPOINT }>);
        idt.general_protection_fault
            .register(ForwardRing3Exceptions::<{ ExceptionKind::GENERAL_PROTECTION }>);
        idt.overflow
            .register(ForwardRing3Exceptions::<{ ExceptionKind::OVERFLOW }>);
        idt.divide_error
            .register(ForwardRing3Exceptions::<{ ExceptionKind::DIVIDE_ERROR }>);
        idt.non_maskable_interrupt
            .register(ForwardRing3Exceptions::<{ ExceptionKind::NON_MASKABLE_INTERRUPT }>);
        idt.bound_range_exceeded
            .register(ForwardRing3Exceptions::<{ ExceptionKind::BOUND_RANGE_EXCEEDED }>);
        idt.debug
            .register(ForwardRing3Exceptions::<{ ExceptionKind::DEBUG }>);
        idt.invalid_opcode
            .register(ForwardRing3Exceptions::<{ ExceptionKind::INVALID_OP_CODE }>);
        idt.device_not_available
            .register(ForwardRing3Exceptions::<{ ExceptionKind::DEVICE_NOT_AVAILABLE }>);
        idt.invalid_tss
            .register(ForwardRing3Exceptions::<{ ExceptionKind::INVALID_TSS }>);
        idt.segment_not_present
            .register(ForwardRing3Exceptions::<{ ExceptionKind::SEGMENT_NOT_PRESENT }>);
        idt.stack_segment_fault
            .register(ForwardRing3Exceptions::<{ ExceptionKind::STACK_SEGMENT_FAULT }>);
        idt.x87_floating_point
            .register(ForwardRing3Exceptions::<{ ExceptionKind::X87_FLOATING_POINT }>);
        idt.alignment_check
            .register(ForwardRing3Exceptions::<{ ExceptionKind::ALIGNMENT_CHECK }>);
        idt.simd_floating_point
            .register(ForwardRing3Exceptions::<{ ExceptionKind::SIMD_FLOATING_POINT }>);
        idt.virtualization
            .register(ForwardRing3Exceptions::<{ ExceptionKind::VIRTUALIZATION }>);
        idt.vmm_communication_exception
            .register(ForwardRing3Exceptions::<{ ExceptionKind::VMM_COMMUNICATION_EXCEPTION }>);
        idt.security_exception
            .register(ForwardRing3Exceptions::<{ ExceptionKind::SECURITY_EXCEPTION }>);
        idt.cp_protection_exception
            .register(ForwardRing3Exceptions::<{ ExceptionKind::CP_PROTECTION_EXCEPTION }>);
        idt.hv_injection_exception
            .register(ForwardRing3Exceptions::<{ ExceptionKind::HV_INJECTION_EXCEPTION }>);
        idt.machine_check
            .register(PanicHandler::<{ ExceptionKind::MACHINE_CHECK }>);
        // SAFETY: Stack indeces provided are valid and only used for the specific handlers.
        unsafe {
            idt.double_fault
                .register(PanicHandler::<{ ExceptionKind::DOUBLE_FAULT }>)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
            idt.page_fault
                .register(page_fault_handler)
                .set_stack_index(gdt::PAGE_FAULT_IST_INDEX);
        }
        // SAFETY: The address provided will match the syscall ABI
        idt[SYSCALL_INT]
            .register(SyscallHandler)
            .set_privilege_level(PrivilegeLevel::Ring3);

        idt[PIC_OFFSET].register(IrqHandler::<0>);
        idt[PIC_OFFSET + 1].register(IrqHandler::<1>);
        idt[PIC_OFFSET + 2].register(IrqHandler::<2>);
        idt[PIC_OFFSET + 3].register(IrqHandler::<3>);
        idt[PIC_OFFSET + 4].register(IrqHandler::<4>);
        idt[PIC_OFFSET + 5].register(IrqHandler::<5>);
        idt[PIC_OFFSET + 6].register(IrqHandler::<6>);
        idt[PIC_OFFSET + 7].register(IrqHandler::<7>);
        idt[PIC_OFFSET + 8].register(IrqHandler::<8>);
        idt[PIC_OFFSET + 9].register(IrqHandler::<9>);
        idt[PIC_OFFSET + 10].register(IrqHandler::<10>);
        idt[PIC_OFFSET + 11].register(IrqHandler::<11>);
        idt[PIC_OFFSET + 12].register(IrqHandler::<12>);
        idt[PIC_OFFSET + 13].register(IrqHandler::<13>);
        idt[PIC_OFFSET + 14].register(IrqHandler::<14>);
        idt
    });
    IDT.load();
}

pub fn init_irqs() {
    let pics = PICS.bind().expect("Unable to bind PICS to init core");
    IRQ_CTRL_TABLE
        .bind_to_core()
        .expect("Unable to bind IRQ ctrl table to init core");
    // SAFETY: We have exclusive ownership of the PICS
    unsafe {
        pics.lock().initialize();
        pics.lock().write_masks(0xFC, 0xFF);
    }
}

#[inline(always)]
fn page_fault_handler(mut ctx: ExceptionCtx<Exception>) {
    let code = PageFaultErrorCode::from_bits(ctx.error_code()).unwrap();
    let addr = Cr2::read().unwrap().as_ptr::<()>() as usize;
    // SAFETY: Only safe handling of the RIP for fix-up logic in case of userspace pointer reads
    let isr_stack = unsafe { ctx.interrupt_stack_frame_mut() };
    match isr_stack.code_segment.rpl() {
        PrivilegeLevel::Ring0 => match VirtAddr::new(addr).memory_segment() {
            MemorySegment::User => {
                if *CORE_LOCAL_SAFE_BUFFER_LOCK.get() {
                    isr_stack.instruction_pointer =
                        VirtAddrImpl::new(user_buffer_read_page_fault_call_gate as usize as u64);
                } else {
                    panic!(
                        "PAGE FAULT (attempted user read without safeguard) @ {addr:#X?} - ({code:?}) {ctx:#?}"
                    );
                }
                // Trying to safely read user pointer
            }
            MemorySegment::Kernel | MemorySegment::Untyped => {
                panic!("PAGE FAULT @ {addr:#X?} - ({code:?}) {ctx:#?}");
            }
        },
        PrivilegeLevel::Ring1 | PrivilegeLevel::Ring2 => {
            unreachable!("Unexpected ring usage in dual mode processor use")
        }
        PrivilegeLevel::Ring3 => {
            log::debug!("Ring3 Page fault handling due to access at @ {addr:#X?}");
            ring3_exception_handler(
                ExceptionArgs {
                    kind: ExceptionKind::PageFault as usize,
                    code: ctx.error_code(),
                    extra: addr as u64,
                },
                IrqCtx::Exception(ctx),
            )
        }
    }
}
