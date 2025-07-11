mod handlers;

use handlers::{Isr, PanicHandler};
use sync::cell::AtomicLazyCell;
use x86_64::{
    PrivilegeLevel,
    registers::control::Cr2,
    structures::idt::{InterruptDescriptorTable, PageFaultErrorCode},
};

use crate::x86_64_impl::{
    exec::{Exception, ExceptionCtx},
    gdt,
};

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
        idt[SYSCALL_INT]
            .register(PanicHandler::<21>)
            .set_privilege_level(PrivilegeLevel::Ring3);
        idt
    });
    IDT.load();
}

#[inline(always)]
fn page_fault_handler(ctx: ExceptionCtx<Exception>) {
    let code = PageFaultErrorCode::from_bits(ctx.error_code()).unwrap();
    let addr = Cr2::read().unwrap().as_ptr::<()>() as usize;
    panic!("PAGE FAULT @ {addr:#X} - ({code:?}) {ctx:#?}");
}
