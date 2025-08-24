use core::arch::asm;
use core::borrow::Borrow;

use exec::{ExceptionAbi, ExecCtx, IrqCtx};
use mem_impl::page_table::{AnyPageTable, PageTableOffset, X64Addrspace};
use qapi::caps::{CapError, PositiveIsize};
use qapi::mem::vmtable::VMTableEntry;
use qapi::syscall::ops::ctable::VMTableCons;
use qapi::syscall::ops::introspect::{IntrospectResult, VMTable};
use qapi::syscall::ops::vmtable::PaddedPageTableOffset;
use x86_64::instructions::interrupts;
use x86_64::registers::model_specific::GsBase;
use x86_64::structures::idt::InterruptStackFrameValue;

use crate::arch::Addrspace as _;

pub mod exec;
mod gdt;
mod idt;
mod mem_impl;

use crate::arch::mem::VirtAddr;
use crate::arch::System;
use crate::kmem::KPtr;
use crate::syscall::SyscallResp;
use crate::PMO;

use super::mem::PageFlags;

#[derive(Debug)]
pub struct X64Sys {}

// SAFETY: The system trait implementation is aaccurate for x86-64 systems.
unsafe impl System for X64Sys {
    type Addrspace = X64Addrspace;
    type ExecState = ExecCtx;
    type PageTable = AnyPageTable;
    type ArchCaps = ArchCaps;
    type IrqCtx = IrqCtx;
    type ExceptionAbi = ExceptionAbi;

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

    fn post_init(&self) {
        X64Addrspace::current(*PMO).obscure_top_half();
    }

    fn set_core_data(&self, addr: VirtAddr) {
        debug_assert!(addr.is_higher_half());
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
#[non_exhaustive]
pub enum ArchCaps {
    L4(KPtr<AnyPageTable>),
    L3(KPtr<AnyPageTable>),
    L2(KPtr<AnyPageTable>),
    L1(KPtr<AnyPageTable>),
}

#[allow(unreachable_patterns)]
impl super::ArchCaps<X64Sys> for ArchCaps {
    fn new_vmtable(args: VMTableCons) -> Result<Self, CapError> {
        let table = KPtr::new(args.frame.into(), AnyPageTable::new())?;
        let cap = match args.level {
            1 => Self::L1(table),
            2 => Self::L2(table),
            3 => Self::L3(table),
            4 => Self::L4(table),
            _ => return Err(CapError::InvalidVMTableLevel),
        };
        Ok(cap)
    }
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

    fn as_vmtable(&self) -> Result<&KPtr<<X64Sys as System>::PageTable>, CapError> {
        match self {
            ArchCaps::L4(kptr) => Ok(kptr),
            ArchCaps::L3(kptr) => Ok(kptr),
            ArchCaps::L2(kptr) => Ok(kptr),
            ArchCaps::L1(kptr) => Ok(kptr),
            _ => Err(CapError::InvalidCapType),
        }
    }

    fn vm_link(
        top_table: &Self,
        slot: PaddedPageTableOffset,
        bottom_table: &Self,
        flags: PageFlags,
    ) -> SyscallResp {
        let (top_table, top_level) = match top_table {
            ArchCaps::L4(kptr) => (kptr, 4),
            ArchCaps::L3(kptr) => (kptr, 3),
            ArchCaps::L2(kptr) => (kptr, 2),
            ArchCaps::L1(kptr) => (kptr, 1),
            _ => return Err(CapError::InvalidCapType),
        };
        let (bottom_table, bottom_level) = match bottom_table {
            ArchCaps::L4(kptr) => (kptr, 4),
            ArchCaps::L3(kptr) => (kptr, 3),
            ArchCaps::L2(kptr) => (kptr, 2),
            ArchCaps::L1(kptr) => (kptr, 1),
            _ => return Err(CapError::InvalidCapType),
        };

        if top_level != bottom_level + 1 {
            return Err(CapError::VMLinkNotFlat);
        }
        let slot: usize = slot.into();
        let slot = slot.try_into()?;

        // SAFETY: This can't be kernel memory since it comes from a userspace capability
        unsafe {
            top_table.get(slot).set(bottom_table.frame(), flags.into());
        }
        Ok(PositiveIsize::zero())
    }

    fn vm_unlink(table: &Self, slot: PaddedPageTableOffset) -> SyscallResp {
        let table = match table {
            ArchCaps::L4(kptr) => kptr,
            ArchCaps::L3(kptr) => kptr,
            ArchCaps::L2(kptr) => kptr,
            ArchCaps::L1(kptr) => kptr,
            _ => return Err(CapError::InvalidCapType),
        };
        let slot: usize = slot.into();
        let slot = slot.try_into()?;
        // SAFETY: This can't be kernel memory since it comes from a userspace capability
        unsafe {
            table.get(slot).reset();
        }
        Ok(PositiveIsize::zero())
    }
    fn vm_set_attributes(
        table: &Self,
        slot: PaddedPageTableOffset,
        attributes: PageFlags,
    ) -> SyscallResp {
        let table = match table {
            ArchCaps::L4(kptr) => kptr,
            ArchCaps::L3(kptr) => kptr,
            ArchCaps::L2(kptr) => kptr,
            ArchCaps::L1(kptr) => kptr,
            _ => return Err(CapError::InvalidCapType),
        };
        let slot: usize = slot.into();
        let slot = slot.try_into()?;
        // SAFETY: This can't be kernel memory since it comes from a userspace capability
        unsafe {
            table.get(slot).set_flags(attributes.into());
        }
        Ok(PositiveIsize::zero())
    }

    fn introspect(&self) -> IntrospectResult {
        let (level, table) = match self {
            ArchCaps::L4(kptr) => (4, kptr),
            ArchCaps::L3(kptr) => (3, kptr),
            ArchCaps::L2(kptr) => (2, kptr),
            ArchCaps::L1(kptr) => (1, kptr),
        };
        let mut entries = [VMTableEntry::empty(); 512];
        for (i, entry) in entries.iter_mut().enumerate() {
            *entry = table
                .get(PageTableOffset::new(i as u16).unwrap())
                .get_raw()
                .into();
        }

        IntrospectResult::VMTable(VMTable {
            kobj: table.frame().into(),
            level,
            entries,
        })
    }
}
