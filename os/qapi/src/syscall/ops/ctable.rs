use derive_more::TryFrom;
use qapi_macros::SyscallRequest;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::caps::ctable::CTableCap;
use crate::caps::vmtable::VMTableCap;
use crate::caps::{CapId, SysSlot};
use crate::types::UserPtr;

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct ConsOp {
    pub table_cap: CTableCap,
    pub slot_id: SysSlot,
    pub kind: ConsKind,
    pub cons_args: UserPtr<()>,
}

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct DropOp {
    pub table_cap: CTableCap,
    pub slot_id: SysSlot,
}

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct LinkOp {
    pub top_table: CTableCap,
    pub slot_id: SysSlot,
    pub bottom_table: CTableCap,
}

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct CopyOp {
    pub from_cap: CapId,
    pub to_table: CTableCap,
    pub to_slot: SysSlot,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum ConsArgs {
    Thread(ThreadCons),
    VMTable(VMTableCons),
    SyncCall(SyncCallCons),
    CapTable(CapTableCons),
}

#[derive(KnownLayout, IntoBytes, FromBytes, Immutable, Debug, Copy, Clone)]
#[repr(C)]
pub struct ThreadCons {
    pub entry: usize,
    pub rsp: usize,
    pub addrspace: VMTableCap,
    pub caps: CTableCap,
    pub frame: u64,
    pub arg0: usize,
}

#[derive(KnownLayout, IntoBytes, FromBytes, Immutable, Debug, Copy, Clone)]
#[repr(C)]
pub struct VMTableCons {}

#[derive(KnownLayout, IntoBytes, FromBytes, Immutable, Debug, Copy, Clone)]
#[repr(C)]
pub struct SyncCallCons {
    pub entry: usize,
    pub cspace: CTableCap,
    pub vmspace: VMTableCap,
}

#[derive(KnownLayout, IntoBytes, FromBytes, Immutable, Debug, Copy, Clone)]
#[repr(C)]
pub struct CapTableCons {}

#[repr(usize)]
#[derive(Debug, Copy, Clone, TryFrom)]
#[try_from(repr)]
pub enum ConsKind {
    Thread = 0,
    CTable,
    VMTable,
    SyncCall,
}

impl From<ConsKind> for usize {
    fn from(value: ConsKind) -> Self {
        value as usize
    }
}
