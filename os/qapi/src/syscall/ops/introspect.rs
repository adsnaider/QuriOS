use core::mem::MaybeUninit;

use qapi_macros::SyscallRequest;

use crate::caps::CapId;
use crate::mem::Frame;
use crate::mem::vmtable::VMTableEntry;
use crate::types::UserPtrMut;

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
    CLink(KObj<CBlockInspect>),
    Thread(KObj<ThreadInspect>),
    CBlock(KObj<CBlockInspect>),
    SyncCall(SyncCallInspect),
    Resources(ResourcesInspect),
    Notification(NotificationInspect),
    #[cfg(target_arch = "x86_64")]
    VMTable(KObj<VMTableInspect>),
    #[cfg(target_arch = "x86_64")]
    IrqCtrl(IrqCtrlInspect),
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct KObj<T> {
    pub kobj: Frame,
    pub data: T,
}

impl<T> KObj<T> {
    pub const fn new(frame: Frame, data: T) -> Self {
        Self { kobj: frame, data }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IrqCtrlInspect;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CLinkInspect;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SyncCallInspect;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ResourcesInspect;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NotificationInspect;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ThreadInspect;
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CBlockInspect;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VMTableInspect {
    pub level: u8,
    pub entries: [VMTableEntry; 512],
}

impl core::fmt::Debug for VMTableInspect {
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
            .field("level", &self.level)
            .field("entries", &FilteredEntries(&self.entries))
            .finish()
    }
}
