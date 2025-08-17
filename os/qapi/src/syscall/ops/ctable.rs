use derive_more::TryFrom;
use qapi_macros::SyscallRequest;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::caps::ctable::CapTableCap;
use crate::caps::vmtable::PageTableCap;
use crate::caps::{CapId, SysSlot};
use crate::types::UserPtr;

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct ConsOp {
    pub table_cap: CapTableCap,
    pub slot_id: SysSlot,
    pub kind: ConsKind,
    pub cons_args: UserPtr<()>,
}

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct DropOp {
    pub table_cap: CapTableCap,
    pub slot_id: SysSlot,
}

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct LinkOp {
    pub top_table: CapTableCap,
    pub slot_id: SysSlot,
    pub linked_table: CapTableCap,
}

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct UnlinkOp {
    pub top_table: CapTableCap,
    pub slot_id: SysSlot,
}

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct CopyOp {
    pub from_cap: CapId,
    pub to_table: CapTableCap,
    pub to_slot: SysSlot,
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

impl From<ConsKind> for usize {
    fn from(value: ConsKind) -> Self {
        value as usize
    }
}
