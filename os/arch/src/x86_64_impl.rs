use core::arch::asm;

use x86_64::instructions::interrupts;

mod gdt;
mod idt;
mod mem_impl;

use crate::System;

pub struct Sys {}

impl System for Sys {}

impl Sys {
    pub fn init() -> Self {
        interrupts::disable();
        sce_enable();
        gdt::init();
        idt::init();
        unsafe {
            core::ptr::read_volatile(0xffffffff80000000 as *const u8);
        }
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
