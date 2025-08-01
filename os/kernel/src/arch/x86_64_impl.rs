use core::{arch::asm, borrow::Borrow};

use exec::ExecCtx;
use mem_impl::page_table::{AnyPageTable, X64Addrspace};
use qapi::caps::CapError;
use x86_64::{instructions::interrupts, registers::model_specific::GsBase};

mod exec;
mod gdt;
mod idt;
mod mem_impl;

use crate::{
    arch::{mem::VirtAddr, System},
    kmem::KPtr,
    PMO,
};

#[derive(Debug)]
pub struct X64Sys {}

// SAFETY: The system trait implementation is aaccurate for x86-64 systems.
unsafe impl System for X64Sys {
    type Addrspace = X64Addrspace;
    type ExecState = ExecCtx;
    type PageTable = AnyPageTable;
    type ArchCaps = ArchCaps;

    fn addrspace(&self) -> Self::Addrspace {
        // SAFETY: PMO is correct from initialization
        X64Addrspace::current(*PMO)
    }

    fn init() -> Self {
        interrupts::disable();
        sce_enable();
        gdt::init();
        idt::init();
        Self {}
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

#[derive(Debug, Clone)]
pub enum ArchCaps {
    L4(KPtr<AnyPageTable>),
    L3(KPtr<AnyPageTable>),
    L2(KPtr<AnyPageTable>),
    L1(KPtr<AnyPageTable>),
}

impl super::ArchCaps<X64Sys> for ArchCaps {
    fn addrspace<A>(addrspace: A) -> Self
    where
        A: Borrow<X64Addrspace>,
    {
        todo!()
    }

    fn as_addrspace(&self) -> Result<&KPtr<AnyPageTable>, CapError> {
        todo!();
    }
}
