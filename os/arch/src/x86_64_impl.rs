use core::arch::asm;

use x86_64::instructions::interrupts;

mod gdt;
mod mem_impl;

use crate::System;

pub struct Sys {}

impl System for Sys {}

impl Sys {
    pub fn init() -> Self {
        interrupts::disable();
        sce_enable();
        gdt::init();
        // IDT
        // Timer will be initialized in userspace...
        // Sentinel page for interrupt stack overflow
        todo!();
    }
}

fn sce_enable() {
    // SAFETY: Nothing special, just enabling Syscall extension.
    unsafe {
        asm!(
            "mov rcx, 0xc0000082",
            "wrmsr",
            "mov rcx, 0xc0000080",
            "rdmsr",
            "or eax, 1",
            "wrmsr",
            "mov rcx, 0xc0000081",
            "rdmsr",
            "mov edx, 0x00180008",
            "wrmsr",
            out("rcx") _,
            out("eax") _,
            out("edx") _,
            options(nostack, nomem),
        );
    }
    log::info!("Enabled SCE x86-64 extension");
}
