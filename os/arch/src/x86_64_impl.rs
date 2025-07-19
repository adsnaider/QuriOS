use core::arch::asm;

use exec::{ExceptionCtx, ExecCtx, Interrupt};
use mem_impl::page_table::{AnyPageTable, X64Addrspace};
use x86_64::{
    instructions::interrupts,
    registers::{
        control::{Cr4, Cr4Flags},
        model_specific::{GsBase, KernelGsBase},
        segmentation::{GS, Segment64},
    },
};

mod exec;
mod gdt;
mod idt;
mod mem_impl;

use crate::{
    SyscallHandler, System,
    mem::{Pmo, VirtAddr},
};

#[derive(Debug)]
pub struct X64Sys {
    pmo: Pmo,
    syscall_handler: SyscallHandler<Self>,
}

// SAFETY: The system trait implementation is aaccurate for x86-64 systems.
unsafe impl System for X64Sys {
    type Addrspace = X64Addrspace;
    type ExecState = ExecCtx;
    type SyscallCtx = ExceptionCtx<Interrupt>;
    type PageTable = AnyPageTable;

    fn addrspace(&self) -> Self::Addrspace {
        // SAFETY: PMO is correct from initialization
        X64Addrspace::current(self.pmo)
    }

    fn init(pmo: Pmo, syscall_handler: SyscallHandler<Self>) -> Self {
        interrupts::disable();
        sce_enable();
        gdt::init();
        idt::init();
        Self {
            pmo,
            syscall_handler,
        }
    }

    fn set_core_data(&self, addr: VirtAddr) {
        log::info!("Setting GS Base to {addr:?}");
        GsBase::write(addr.into());
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
