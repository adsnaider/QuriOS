use core::sync::atomic::AtomicU16;

use zerocopy::{Immutable, IntoBytes, KnownLayout};

use crate::{
    caps::{CapId, cap_table::CapTable, page_table::Addrspace, thread::Thread},
    types::CSlice,
};

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
    pub self_caps: CapTable,
    pub self_addrspace: Addrspace,
    pub self_thread: Thread,
}

impl Default for BootCaps {
    fn default() -> Self {
        Self::new()
    }
}

impl BootCaps {
    pub const fn new() -> Self {
        Self {
            self_caps: CapTable::new(CapId::new(0)),
            self_addrspace: Addrspace::new(CapId::new(1)),
            self_thread: Thread::new(CapId::new(2)),
        }
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct RetypeEntry(pub AtomicU16);
