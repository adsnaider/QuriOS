mod handlers;

use core::arch::naked_asm;

use handlers::{Isr, PanicHandler};
use qapi::syscall::SyscallArgs;
use qapi::{caps::CapResult as _, syscall::SyscallOp};
use sync::cell::AtomicLazyCell;
use x86_64::{
    registers::control::Cr2,
    structures::idt::{InterruptDescriptorTable, PageFaultErrorCode},
    PrivilegeLevel, VirtAddr as VirtAddrImpl,
};

use crate::arch::mem::{user_buffer_read_page_fault_call_gate, MemorySegment, VirtAddr};
use crate::arch::x86_64_impl::{
    exec::{Exception, ExceptionCtx},
    gdt,
};
use crate::core_local::CORE_LOCAL_SAFE_BUFFER_LOCK;
use crate::syscall::syscall_handler;

use super::exec::Interrupt;

const SYSCALL_INT: u8 = 0x80;

/// Initializes the IDT and sets up the 8259 PIC.
pub fn init() {
    init_idt();
    log::info!("Interrupt tables initialized");
}

/// Initializes the interrupt descriptor table.
fn init_idt() {
    static IDT: AtomicLazyCell<InterruptDescriptorTable> = AtomicLazyCell::new(|| {
        let mut idt = InterruptDescriptorTable::new();
        // Exceptions.
        idt.breakpoint.register(PanicHandler::<0>);
        idt.general_protection_fault.register(PanicHandler::<1>);
        idt.overflow.register(PanicHandler::<2>);
        idt.divide_error.register(PanicHandler::<3>);
        idt.non_maskable_interrupt.register(PanicHandler::<2>);
        idt.bound_range_exceeded.register(PanicHandler::<3>);
        idt.debug.register(PanicHandler::<4>);
        idt.invalid_opcode.register(PanicHandler::<5>);
        idt.device_not_available.register(PanicHandler::<6>);
        idt.invalid_tss.register(PanicHandler::<7>);
        idt.segment_not_present.register(PanicHandler::<8>);
        idt.stack_segment_fault.register(PanicHandler::<9>);
        idt.x87_floating_point.register(PanicHandler::<10>);
        idt.alignment_check.register(PanicHandler::<11>);
        idt.machine_check.register(PanicHandler::<12>);
        idt.simd_floating_point.register(PanicHandler::<13>);
        idt.virtualization.register(PanicHandler::<14>);
        idt.vmm_communication_exception.register(PanicHandler::<15>);
        idt.security_exception.register(PanicHandler::<16>);
        idt.cp_protection_exception.register(PanicHandler::<17>);
        idt.hv_injection_exception.register(PanicHandler::<18>);
        // SAFETY: Stack indeces provided are valid and only used for the specific handlers.
        unsafe {
            idt.double_fault
                .register(PanicHandler::<19>)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
            idt.page_fault
                .register(page_fault_handler)
                .set_stack_index(gdt::PAGE_FAULT_IST_INDEX);
        }
        // SAFETY: The address provided will match the syscall ABI
        unsafe {
            idt[SYSCALL_INT]
                .set_handler_addr(VirtAddrImpl::from_ptr(syscall_int as *const ()))
                .set_privilege_level(PrivilegeLevel::Ring3)
        };
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
        PrivilegeLevel::Ring3 => panic!("PAGE FAULT @ {addr:#X?} - ({code:?}) {ctx:#?}"),
    }
}

#[unsafe(naked)]
extern "C" fn syscall_int(
    cap: usize,
    op: SyscallOp,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
) -> isize {
    extern "C" fn inner(
        cap: usize,
        op: SyscallOp,
        a: usize,
        b: usize,
        c: usize,
        d: usize,
        ctx: ExceptionCtx<Interrupt>,
    ) -> isize {
        // SAFETY: It would be impossible to get to this interrupt handler
        // without having first initialized the IDT with arch::init
        syscall_handler(cap, SyscallArgs::new(op, [a, b, c, d]), ctx).into_isize()
    }
    // SAFETY: Userspace expects syscall interrupt to behave like a C calling convention syscall which
    // will work so long as userspace doesn't need to pass arguments on the stack.
    // Since we filled all the register-args, we need to add the exception context on the stack.
    naked_asm!(
        "swapgs",
        "push rsp",
        "call {inner}",
        "add rsp, 8",
        "swapgs",
        "iretq",
        inner = sym inner,
    )
}
