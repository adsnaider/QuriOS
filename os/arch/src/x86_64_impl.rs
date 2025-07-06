use core::arch::asm;

use exec::SyscallCtx;
use x86_64::instructions::interrupts;

mod exec;
mod gdt;
mod idt;
mod mem_impl;

use crate::System;

pub struct Sys {}

// SAFETY: The system trait implementation is aaccurate for x86-64 systems.
unsafe impl System for Sys {
    unsafe fn syscall_ctx(&self) -> impl crate::SysCtx {
        // SAFETY: Precondition
        unsafe { SyscallCtx::current() }
    }
}

impl Sys {
    pub fn init() -> Self {
        interrupts::disable();
        sce_enable();
        gdt::init();
        idt::init();
        Self {}
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
