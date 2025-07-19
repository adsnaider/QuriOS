//! Capability operations on a capability table enabling a user-space capability system

use core::mem::MaybeUninit;

use derive_more::{From, Into, TryFrom};
use trie::SlotId;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::{
    syscall::{SyscallArgs, SyscallArgsInit, SyscallArgsUninit, SyscallOp, SyscallStruct},
    types::UserPtr,
};

#[cfg(feature = "userspace")]
pub mod ulib;

use super::{CapError, CapId, NUM_SLOTS, page_table::Addrspace};

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
pub struct CapTable(CapId);

#[derive(Debug, Copy, Clone)]
pub struct ConsOp {
    pub slot_id: SlotId<NUM_SLOTS>,
    pub kind: ConsKind,
    pub cons_args: UserPtr<()>,
}

impl SyscallStruct for ConsOp {
    fn into_args(self) -> SyscallArgs<SyscallArgsUninit> {
        let mut sysargs = SyscallArgs::new_uninit(SyscallOp::CAP_TABLE_CONS);
        let args = sysargs.args_mut();
        args[0] = MaybeUninit::new(self.slot_id.into());
        args[1] = MaybeUninit::new(self.kind as usize);
        args[2] = MaybeUninit::new(self.cons_args.addr());
        sysargs
    }

    fn try_from_args(args: SyscallArgs<SyscallArgsInit>) -> Result<Self, CapError> {
        if args.op() != SyscallOp::CAP_TABLE_CONS {
            return Err(CapError::InvalidOp);
        }
        let args = args.args();
        Ok(Self {
            slot_id: SlotId::new(args[0])?,
            kind: args[1].try_into().map_err(|_| CapError::InvalidArg)?,
            cons_args: UserPtr::from_addr(args[2]),
        })
    }
}

impl CapTable {
    pub const fn new(cap: CapId) -> Self {
        Self(cap)
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
    pub addrspace: Addrspace,
    pub caps: CapTable,
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

#[repr(usize)]
#[derive(Debug, Copy, Clone)]
pub enum CapTableOps {
    Cons(ConsOp) = SyscallOp::CAP_TABLE_CONS.as_usize(),
}

impl SyscallStruct for CapTableOps {
    fn into_args(self) -> SyscallArgs<SyscallArgsUninit> {
        match self {
            CapTableOps::Cons(cons_op) => cons_op.into_args(),
        }
    }

    fn try_from_args(args: SyscallArgs<SyscallArgsInit>) -> Result<Self, CapError> {
        match args.op() {
            SyscallOp::CAP_TABLE_CONS => Ok(Self::Cons(ConsOp::try_from_args(args)?)),
            _ => Err(CapError::InvalidOp),
        }
    }
}
