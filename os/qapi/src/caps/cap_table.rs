//! Capability operations on a capability table enabling a user-space capability system

use core::mem::MaybeUninit;

use derive_more::{From, Into, TryFrom};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::{
    syscall::{InitSyscallParams, SYSCALL_ARGS, UninitSyscallParams},
    types::UserPtr,
};

#[cfg(feature = "userspace")]
pub mod ulib;

use super::{CapError, CapId, NUM_SLOTS, SlotId, page_table::PageTableCap};

#[repr(transparent)]
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    From,
    Into,
    KnownLayout,
    IntoBytes,
    FromBytes,
    Immutable,
)]
pub struct CapTableCap(CapId);

#[derive(Debug, Copy, Clone)]
pub struct ConsOp {
    pub table_cap: CapId,
    pub slot_id: SlotId<NUM_SLOTS>,
    pub kind: ConsKind,
    pub cons_args: UserPtr<()>,
}

impl ConsOp {
    pub fn into_args(self) -> UninitSyscallParams {
        let mut args = [MaybeUninit::uninit(); SYSCALL_ARGS];
        args[0] = MaybeUninit::new(self.table_cap.into());
        args[1] = MaybeUninit::new(self.slot_id.into());
        args[2] = MaybeUninit::new(self.kind as usize);
        args[3] = MaybeUninit::new(self.cons_args.addr());
        args
    }

    pub fn try_from_args(args: &InitSyscallParams) -> Result<Self, CapError> {
        Ok(Self {
            table_cap: CapId::try_from(args[0])?,
            slot_id: SlotId::new(args[1])?,
            kind: args[2].try_into().map_err(|_| CapError::InvalidArg)?,
            cons_args: UserPtr::from_addr(args[3]),
        })
    }
}

impl CapTableCap {
    pub const fn new(cap: CapId) -> Self {
        Self(cap)
    }

    pub const fn cap(&self) -> CapId {
        self.0
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum ConsArgs {
    Thread(ThreadCons),
    TranscientPageTable(PageTableCons),
    Addrspace(AddrspaceCons),
    SyncCall(SyncCallCons),
    SyncRet(SyncRetCons),
    CapTable(CapTableCons),
}

#[derive(KnownLayout, IntoBytes, FromBytes, Immutable, Debug, Copy, Clone)]
#[repr(C)]
pub struct ThreadCons {
    pub entry: usize,
    pub rsp: usize,
    pub addrspace: PageTableCap,
    pub caps: CapTableCap,
    pub frame: u64,
    pub arg0: usize,
}

#[derive(KnownLayout, IntoBytes, FromBytes, Immutable, Debug, Copy, Clone)]
#[repr(C)]
pub struct PageTableCons {}
#[derive(KnownLayout, IntoBytes, FromBytes, Immutable, Debug, Copy, Clone)]
#[repr(C)]
pub struct AddrspaceCons {}
#[derive(KnownLayout, IntoBytes, FromBytes, Immutable, Debug, Copy, Clone)]
#[repr(C)]
pub struct SyncCallCons {}
#[derive(KnownLayout, IntoBytes, FromBytes, Immutable, Debug, Copy, Clone)]
#[repr(C)]
pub struct SyncRetCons {}
#[derive(KnownLayout, IntoBytes, FromBytes, Immutable, Debug, Copy, Clone)]
#[repr(C)]
pub struct CapTableCons {}

#[repr(usize)]
#[derive(Debug, Copy, Clone, TryFrom)]
#[try_from(repr)]
pub enum ConsKind {
    Thread = 0,
    CapTable,
    TranscientPageTable,
    Addrspace,
    SyncCall,
    SyncRet,
}
