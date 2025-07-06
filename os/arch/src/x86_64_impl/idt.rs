mod handlers;

use handlers::PanicHandler;
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
        PanicHandler::set_handler(&mut idt.breakpoint);
        PanicHandler::set_handler(&mut idt.general_protection_fault);
        PanicHandler::set_handler(&mut idt.overflow);
        PanicHandler::set_handler(&mut idt.divide_error);
        PanicHandler::set_handler(&mut idt.non_maskable_interrupt);
        PanicHandler::set_handler(&mut idt.bound_range_exceeded);
        PanicHandler::set_handler(&mut idt.debug);
        PanicHandler::set_handler(&mut idt.invalid_opcode);
        PanicHandler::set_handler(&mut idt.device_not_available);
        PanicHandler::set_handler(&mut idt.invalid_tss);
        PanicHandler::set_handler(&mut idt.segment_not_present);
        PanicHandler::set_handler(&mut idt.stack_segment_fault);
        PanicHandler::set_handler(&mut idt.x87_floating_point);
        PanicHandler::set_handler(&mut idt.alignment_check);
        PanicHandler::set_handler(&mut idt.machine_check);
        PanicHandler::set_handler(&mut idt.simd_floating_point);
        PanicHandler::set_handler(&mut idt.virtualization);
        PanicHandler::set_handler(&mut idt.vmm_communication_exception);
        PanicHandler::set_handler(&mut idt.security_exception);
        PanicHandler::set_handler(&mut idt.cp_protection_exception);
        PanicHandler::set_handler(&mut idt.hv_injection_exception);
        // SAFETY: Stack indeces provided are valid and only used for the specific handlers.
        unsafe {
            PanicHandler::set_handler(&mut idt.double_fault)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
            PanicHandler::set_handler(&mut idt.page_fault)
                .set_stack_index(gdt::PAGE_FAULT_IST_INDEX);
        }
        PanicHandler::set_handler(&mut idt[SYSCALL_INT]).set_privilege_level(PrivilegeLevel::Ring3);
        idt
    });
    IDT.load();
}
