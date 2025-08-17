use core::arch::asm;
use core::borrow::Borrow;

use exec::ExecCtx;
use mem_impl::page_table::{AnyPageTable, X64Addrspace};
use qapi::caps::CapError;
use x86_64::instructions::interrupts;
use x86_64::registers::model_specific::GsBase;

use crate::arch::Addrspace as _;

mod exec;
mod gdt;
mod idt;
mod mem_impl;

use crate::arch::mem::VirtAddr;
use crate::arch::System;
use crate::kmem::KPtr;
use crate::PMO;

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
    fn new_addrspace<A>(addrspace: A) -> Self
    where
        A: Borrow<X64Addrspace>,
    {
        let frame = addrspace.borrow().frame();
        // SAFETY: We can use KPtr<AnyPageTable> from an addrspace frame.
        Self::L4(unsafe { KPtr::from_frame_unchecked(frame.try_clone().unwrap()) })
    }

    fn as_addrspace(&self) -> Result<&KPtr<AnyPageTable>, CapError> {
        match self {
            Self::L4(addrspace) => Ok(addrspace),
            _ => Err(CapError::InvalidArg),
        }
    }
}
