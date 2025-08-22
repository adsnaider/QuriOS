use qapi::syscall::ops::vmtable::{VMSetAttr, VMUnlinkOp};
use qapi::{caps::CapError, syscall::ops::vmtable::VMLinkOp};

use crate::arch::ArchCaps;

use crate::{
    arch::{ArchSystem, System},
    thread::Thread,
};

use super::SyscallResp;

pub fn vm_link(opts: VMLinkOp) -> SyscallResp {
    let top_table = Thread::with_current(|thread| thread.get_cap(opts.top_table.cap()))
        .unwrap()
        .ok_or(CapError::CapNotFound)?;
    let top_table = top_table.as_arch_cap()?;

    let bottom_table = Thread::with_current(|thread| thread.get_cap(opts.bottom_table.cap()))
        .unwrap()
        .ok_or(CapError::CapNotFound)?;
    let bottom_table = bottom_table.as_arch_cap()?;

    <ArchSystem as System>::ArchCaps::vm_link(
        top_table,
        opts.offset,
        bottom_table,
        opts.flags.into(),
    )
}

pub fn vm_unlink(opts: VMUnlinkOp) -> SyscallResp {
    let table = Thread::with_current(|thread| thread.get_cap(opts.table.cap()))
        .unwrap()
        .ok_or(CapError::CapNotFound)?;
    let table = table.as_arch_cap()?;
    <ArchSystem as System>::ArchCaps::vm_unlink(table, opts.offset)
}
pub fn vm_set_attr(opts: VMSetAttr) -> SyscallResp {
    let table = Thread::with_current(|thread| thread.get_cap(opts.table.cap()))
        .unwrap()
        .ok_or(CapError::CapNotFound)?;
    let table = table.as_arch_cap()?;
    <ArchSystem as System>::ArchCaps::vm_set_attributes(table, opts.offset, opts.flags.into())
}
