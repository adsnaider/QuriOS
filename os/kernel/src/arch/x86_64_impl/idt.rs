mod handlers;

use handlers::{Isr, IsrHandler, PanicHandler};
use qapi::caps::CapResult;
use qapi::exception::ExceptionKind;
use qapi::syscall::{SyscallArgs, SyscallOp};
use sync::cell::AtomicLazyCell;
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptDescriptorTable, PageFaultErrorCode};
use x86_64::{PrivilegeLevel, VirtAddr as VirtAddrImpl};

use super::exec::Interrupt;
use crate::arch::mem::{user_buffer_read_page_fault_call_gate, MemorySegment, VirtAddr};
use crate::arch::x86_64_impl::exec::{Exception, ExceptionCtx};
use crate::arch::x86_64_impl::gdt;
use crate::core_local::CORE_LOCAL_SAFE_BUFFER_LOCK;
use crate::syscall::{ring3_exception_handler, syscall_handler};

const SYSCALL_INT: u8 = 0x80;

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
            PrivilegeLevel::Ring3 => {
                ring3_exception_handler(ExceptionKind::try_from(ID).unwrap(), None, ctx)
            }
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
                ExceptionKind::try_from(ID).unwrap(),
                Some(ctx.error_code() as usize),
                ctx.downcast(),
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
                let res = syscall_handler(args, ctx);
                log::info!("Syscall response: {res:?}");
                // SAFETY: Done handling syscall so we have exclusive asccess to the interrupt stack frame
                unsafe { ctx.scratch_regs_mut().rax = res.into_isize() as u64 };
            }
        }
    }
}

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
        idt
    });
    IDT.load();
}

#[inline(always)]
fn page_fault_handler(mut ctx: ExceptionCtx<Exception>) {
    let code = PageFaultErrorCode::from_bits(ctx.error_code()).unwrap();
    let addr = Cr2::read().unwrap().as_ptr::<()>() as usize;
    let addr = VirtAddr::new(addr);
    // SAFETY: Only safe handling of the RIP for fix-up logic in case of userspace pointer reads
    let isr_stack = unsafe { ctx.interrupt_stack_frame_mut() };
    match isr_stack.code_segment.rpl() {
        PrivilegeLevel::Ring0 => match addr.memory_segment() {
            MemorySegment::User => {
                // SAFETY: NOT SAFE - TODO CoreLocal
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
        PrivilegeLevel::Ring1 => unreachable!(),
        PrivilegeLevel::Ring2 => unreachable!(),
        PrivilegeLevel::Ring3 => ring3_exception_handler(
            ExceptionKind::PageFault,
            Some(ctx.error_code() as usize),
            ctx.downcast(),
        ),
    }
}
