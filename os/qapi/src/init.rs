use core::sync::atomic::AtomicU16;

use loader::LoadedMagic;
use zerocopy::{Immutable, IntoBytes, KnownLayout};

use crate::caps::ctable::CTableCap;
use crate::caps::irq_ctrl::IrqCtrlCap;
use crate::caps::resources::ResourcesCap;
use crate::caps::thread::ThreadCap;
use crate::caps::vmtable::VMTableCap;
use crate::caps::{CapId, SysSlot};
use crate::types::CSlice;

pub const EXCEPTION_HANDLER_MAGIC: LoadedMagic = LoadedMagic::new([
    0x2e5d95e08702dd36,
    0x4207b064f9f00f9c,
    0xd0ad5a8dd5418e44,
    0xc7ec052989c7d4f6,
]);

pub type EntryFn = extern "C" fn(args: &'static BootArgs) -> !;

#[repr(C)]
#[derive(Debug, IntoBytes, KnownLayout, Immutable)]
pub struct BootArgs {
    pub memory_map: CSlice<'static, RetypeEntry>,
    pub initrd: CSlice<'static, u8>,
    pub free_space_start: usize,
}

impl BootArgs {
    pub fn new(
        memory_map: CSlice<'static, RetypeEntry>,
        initrd: CSlice<'static, u8>,
        free_space_start: usize,
    ) -> Self {
        Self {
            memory_map,
            initrd,
            free_space_start,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct BootCaps {
    pub self_resources: ResourcesCap,
    pub self_caps: CTableCap,
    pub self_addrspace: VMTableCap,
    pub self_thread: ThreadCap,
    pub irq_ctrl: IrqCtrlCap,
}

impl Default for BootCaps {
    fn default() -> Self {
        Self::new()
    }
}

impl BootCaps {
    pub const fn new() -> Self {
        Self {
            self_resources: ResourcesCap::new(CapId::new(0)),
            self_caps: CTableCap::new(CapId::new(1)),
            self_addrspace: VMTableCap::new(CapId::new(2)),
            self_thread: ThreadCap::new(CapId::new(3)),
            irq_ctrl: IrqCtrlCap::new(CapId::new(4)),
        }
    }

    pub const fn next_free() -> SysSlot {
        match SysSlot::new(3) {
            Ok(s) => s,
            Err(_) => unreachable!(),
        }
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct RetypeEntry(pub AtomicU16);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RetypeState {
    Untyped,
    Kernel { refs: u16 },
    User { refs: u16 },
    Unavailable,
}

impl RetypeEntry {
    const STATE_BITS: u16 = 2;
    const COUNTER_BITS: u16 = 16 - Self::STATE_BITS;
    pub const MAX_REF_COUNT: u16 = (1 << Self::COUNTER_BITS) - 1;

    pub fn state(&self) -> RetypeState {
        let value = self.0.load(core::sync::atomic::Ordering::Relaxed);
        let counter = value & ((1 << Self::COUNTER_BITS) - 1);
        match value >> Self::COUNTER_BITS as u8 {
            0 => RetypeState::Unavailable,
            1 => {
                assert_eq!(counter, 0);
                RetypeState::Untyped
            }
            2 => RetypeState::User { refs: counter },
            3 => RetypeState::Kernel { refs: counter },
            _ => panic!("Unexpected kernel retype entry: {value:#x}"),
        }
    }
}
