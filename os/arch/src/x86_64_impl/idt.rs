mod handlers;

use handlers::{DivergingErrCodeIsr, DivergingIsr, ErrCodeIsr, Isr, PanicHandler};
use sync::cell::AtomicLazyCell;
use x86_64::{PrivilegeLevel, structures::idt::InterruptDescriptorTable};

use crate::x86_64_impl::gdt;

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
        Isr::register(&mut idt.breakpoint, PanicHandler::<0>);
        ErrCodeIsr::register(&mut idt.general_protection_fault, PanicHandler::<1>);
        Isr::register(&mut idt.overflow, PanicHandler::<2>);
        Isr::register(&mut idt.divide_error, PanicHandler::<3>);
        Isr::register(&mut idt.non_maskable_interrupt, PanicHandler::<2>);
        Isr::register(&mut idt.bound_range_exceeded, PanicHandler::<3>);
        Isr::register(&mut idt.debug, PanicHandler::<4>);
        Isr::register(&mut idt.invalid_opcode, PanicHandler::<5>);
        Isr::register(&mut idt.device_not_available, PanicHandler::<6>);
        ErrCodeIsr::register(&mut idt.invalid_tss, PanicHandler::<7>);
        ErrCodeIsr::register(&mut idt.segment_not_present, PanicHandler::<8>);
        ErrCodeIsr::register(&mut idt.stack_segment_fault, PanicHandler::<9>);
        Isr::register(&mut idt.x87_floating_point, PanicHandler::<10>);
        ErrCodeIsr::register(&mut idt.alignment_check, PanicHandler::<11>);
        DivergingIsr::register(&mut idt.machine_check, PanicHandler::<12>);
        Isr::register(&mut idt.simd_floating_point, PanicHandler::<13>);
        Isr::register(&mut idt.virtualization, PanicHandler::<14>);
        ErrCodeIsr::register(&mut idt.vmm_communication_exception, PanicHandler::<15>);
        ErrCodeIsr::register(&mut idt.security_exception, PanicHandler::<16>);
        ErrCodeIsr::register(&mut idt.cp_protection_exception, PanicHandler::<17>);
        Isr::register(&mut idt.hv_injection_exception, PanicHandler::<18>);
        // SAFETY: Stack indeces provided are valid and only used for the specific handlers.
        unsafe {
            DivergingErrCodeIsr::register(&mut idt.double_fault, PanicHandler::<19>)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
            ErrCodeIsr::register_page_fault(&mut idt.page_fault, PanicHandler::<20>)
                .set_stack_index(gdt::PAGE_FAULT_IST_INDEX);
        }
        Isr::register(&mut idt[SYSCALL_INT], PanicHandler::<21>)
            .set_privilege_level(PrivilegeLevel::Ring3);
        idt
    });
    IDT.load();
}
