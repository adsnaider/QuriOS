use core::arch::asm;
use core::borrow::Borrow;

use exec::{ExecCtx, IrqCtx};
use idt::IRQ_CTRL_TABLE;
use mem_impl::page_table::{AnyPageTable, PageTableOffset, X64Addrspace};
use qapi::caps::sync_ipc::ExceptionAbi;
use qapi::caps::{CapError, PositiveIsize};
use qapi::mem::vmtable::VMTableEntry;
use qapi::syscall::ops::ctable::VMTableCons;
use qapi::syscall::ops::introspect::{IntrospectResult, VMTable};
use qapi::syscall::ops::vmtable::PaddedPageTableOffset;
use x86_64::instructions::interrupts;
use x86_64::registers::model_specific::GsBase;

use crate::arch::Addrspace as _;
use crate::notify::Notification;

pub mod backtrace;
pub mod exec;
mod gdt;
mod idt;
mod mem_impl;

use super::mem::PageFlags;
use crate::PMO;
use crate::arch::System;
use crate::arch::mem::VirtAddr;
use crate::kmem::KPtr;
use crate::syscall::SyscallResp;

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
        idt::init_irqs();
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
    VMTableL4(KPtr<AnyPageTable>),
    VMTableL3(KPtr<AnyPageTable>),
    VMTableL2(KPtr<AnyPageTable>),
    VMTableL1(KPtr<AnyPageTable>),
    IrqCtrl,
}

#[allow(unreachable_patterns)]
impl super::ArchCaps<X64Sys> for ArchCaps {
    fn new_vmtable(args: VMTableCons) -> Result<Self, CapError> {
        let table = KPtr::new(args.frame.into(), AnyPageTable::new())?;
        let cap = match args.level {
            1 => Self::VMTableL1(table),
            2 => Self::VMTableL2(table),
            3 => Self::VMTableL3(table),
            4 => Self::VMTableL4(table),
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
        Self::VMTableL4(unsafe { KPtr::from_frame_unchecked(frame.try_clone().unwrap()) })
    }

    fn as_addrspace(&self) -> Result<&KPtr<AnyPageTable>, CapError> {
        match self {
            Self::VMTableL4(addrspace) => Ok(addrspace),
            _ => Err(CapError::InvalidArg),
        }
    }

    fn as_vmtable(&self) -> Result<&KPtr<<X64Sys as System>::PageTable>, CapError> {
        match self {
            ArchCaps::VMTableL4(kptr) => Ok(kptr),
            ArchCaps::VMTableL3(kptr) => Ok(kptr),
            ArchCaps::VMTableL2(kptr) => Ok(kptr),
            ArchCaps::VMTableL1(kptr) => Ok(kptr),
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
            ArchCaps::VMTableL4(kptr) => (kptr, 4),
            ArchCaps::VMTableL3(kptr) => (kptr, 3),
            ArchCaps::VMTableL2(kptr) => (kptr, 2),
            ArchCaps::VMTableL1(kptr) => (kptr, 1),
            _ => return Err(CapError::InvalidCapType),
        };
        let (bottom_table, bottom_level) = match bottom_table {
            ArchCaps::VMTableL4(kptr) => (kptr, 4),
            ArchCaps::VMTableL3(kptr) => (kptr, 3),
            ArchCaps::VMTableL2(kptr) => (kptr, 2),
            ArchCaps::VMTableL1(kptr) => (kptr, 1),
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
            ArchCaps::VMTableL4(kptr) => kptr,
            ArchCaps::VMTableL3(kptr) => kptr,
            ArchCaps::VMTableL2(kptr) => kptr,
            ArchCaps::VMTableL1(kptr) => kptr,
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
            ArchCaps::VMTableL4(kptr) => kptr,
            ArchCaps::VMTableL3(kptr) => kptr,
            ArchCaps::VMTableL2(kptr) => kptr,
            ArchCaps::VMTableL1(kptr) => kptr,
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
        match self {
            ArchCaps::VMTableL4(kptr) => Self::introspect_vmtable(4, kptr),
            ArchCaps::VMTableL3(kptr) => Self::introspect_vmtable(3, kptr),
            ArchCaps::VMTableL2(kptr) => Self::introspect_vmtable(2, kptr),
            ArchCaps::VMTableL1(kptr) => Self::introspect_vmtable(1, kptr),
            ArchCaps::IrqCtrl => IntrospectResult::IrqCtrl,
        }
    }

    fn irq_ctrl() -> Result<Self, CapError> {
        Ok(Self::IrqCtrl)
    }

    fn irq_set(&self, irq: usize, notification: Option<Notification<X64Sys>>) -> SyscallResp {
        let ArchCaps::IrqCtrl = self else {
            return Err(CapError::InvalidCapType)?;
        };
        let irq = irq.try_into().map_err(|_| CapError::InvalidArg)?;
        IRQ_CTRL_TABLE
            .set(irq, notification)
            .map_err(|_| CapError::IrqTableBindInvalid)?;
        Ok(PositiveIsize::zero())
    }
}

impl ArchCaps {
    fn introspect_vmtable(level: u8, table: &KPtr<AnyPageTable>) -> IntrospectResult {
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
