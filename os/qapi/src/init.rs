use core::sync::atomic::AtomicU16;

use crate::types::CSlice;

pub type EntryFn = extern "C" fn(args: &'static BootArgs) -> !;

#[repr(C)]
#[derive(Debug)]
pub struct BootArgs {
    pub memory_map: CSlice<'static, RetypeEntry>,
    pub initrd: CSlice<'static, u8>,
    pub free_space_start: usize,
}

impl BootArgs {
    pub fn as_bytes(&self) -> &[u8] {
        todo!();
    }
}

#[derive(Debug)]
pub struct RetypeEntry(AtomicU16);
