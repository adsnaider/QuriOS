//! Capability operations on a capability table enabling a user-space capability system

use core::mem::MaybeUninit;

use derive_more::{From, Into, TryFrom};
use trie::SlotId;

use crate::{
    syscall::{SyscallArgs, SyscallArgsInit, SyscallArgsUninit, SyscallStruct},
    types::OrphanPtr,
};

use super::{CapError, CapId, NUM_SLOTS, page_table::Addrspace};

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into)]
pub struct CapTable(CapId);

#[derive(Debug, Copy, Clone)]
struct ConsArgs {
    cap: CapId,
    slot_id: SlotId<NUM_SLOTS>,
    kind: ConsKind,
    cons_args: OrphanPtr<()>,
}

impl SyscallStruct for ConsArgs {
    fn into_args(self) -> SyscallArgs<SyscallArgsUninit> {
        let mut sysargs = SyscallArgs::new_uninit(self.cap.into());
        let args = sysargs.args_mut();
        args[0] = MaybeUninit::new(self.slot_id.into());
        args[1] = MaybeUninit::new(self.kind as usize);
        args[2] = MaybeUninit::new(self.cons_args.addr());
        sysargs
    }

    fn try_from_args(args: SyscallArgs<SyscallArgsInit>) -> Result<Self, CapError> {
        let cap = args.cap()?;
        let args = args.args();
        Ok(Self {
            cap,
            slot_id: SlotId::new(args[0])?,
            kind: args[1].try_into().map_err(|_| CapError::InvalidArg)?,
            cons_args: OrphanPtr::from_addr(args[2]),
        })
    }
}

impl CapTable {
    pub const fn new(cap: CapId) -> Self {
        Self(cap)
    }

    pub fn construct<T>(&self, capability: T, slot: SlotId<NUM_SLOTS>) -> Result<(), CapError> {
        todo!();
    }
}
#[repr(usize)]
#[derive(Debug, Copy, Clone, TryFrom)]
#[try_from(repr)]
enum ConsKind {
    Thread = 0,
    CapTable,
    TranscientPageTable,
    Addrspace,
    SyncCall,
    SyncRet,
}

pub struct ThreadCons {
    entry: usize,
    rsp: usize,
    addrspace: Addrspace,
    caps: CapTable,
}
