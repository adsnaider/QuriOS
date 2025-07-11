use core::sync::atomic::AtomicU16;

use zerocopy::{Immutable, IntoBytes, KnownLayout};

use crate::types::CSlice;

pub type EntryFn = extern "C" fn(args: &'static BootArgs) -> !;

#[repr(C)]
#[derive(Debug, IntoBytes, KnownLayout, Immutable)]
pub struct BootArgs {
    pub memory_map_ptr: usize,
    pub memory_map_length: usize,
    pub initrd_ptr: usize,
    pub initrd_length: usize,
    pub free_space_start: usize,
}

impl BootArgs {
    pub fn new(
        memory_map: CSlice<'static, RetypeEntry>,
        initrd: CSlice<'static, u8>,
        free_space_start: usize,
    ) -> Self {
        Self {
            memory_map_ptr: memory_map.ptr() as usize,
            memory_map_length: memory_map.len(),
            initrd_ptr: initrd.ptr() as usize,
            initrd_length: initrd.len(),
            free_space_start,
        }
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct RetypeEntry(AtomicU16);
