use core::mem::MaybeUninit;

use qapi_macros::SyscallRequest;

use crate::{
    caps::CapId,
    mem::{Frame, vmtable::VMTableEntry},
    types::UserPtrMut,
};

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct IntrospectOp {
    pub cap: CapId,
    pub write_buf: UserPtrMut<MaybeUninit<IntrospectResult>>,
}

#[repr(C)]
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Copy)]
pub enum IntrospectResult {
    Empty,
    CLink {
        kobj: Frame,
    },
    Thread {
        kobj: Frame,
        thread: Thread,
    },
    CBlock {
        kobj: Frame,
        cblock: CBlock,
    },
    SyncCall,
    SyncRet,
    #[cfg(target_arch = "x86_64")]
    VMTable(VMTable),
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Thread;
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CBlock;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VMTable {
    pub kobj: Frame,
    pub level: u8,
    pub entries: [VMTableEntry; 512],
}

impl core::fmt::Debug for VMTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        struct FilteredEntries<'a>(&'a [VMTableEntry]);
        impl core::fmt::Debug for FilteredEntries<'_> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                let mut lst = f.debug_list();
                for (i, (frame, flags)) in self
                    .0
                    .iter()
                    .enumerate()
                    .filter_map(|(i, entry)| entry.get().map(|entry| (i, entry)))
                {
                    lst.entry(&format_args!("[{i}] => ({frame:?}, {flags:?})"));
                }
                lst.finish()
            }
        }
        f.debug_struct("VMTable")
            .field("kobj", &self.kobj)
            .field("level", &self.level)
            .field("entries", &FilteredEntries(&self.entries))
            .finish()
    }
}
