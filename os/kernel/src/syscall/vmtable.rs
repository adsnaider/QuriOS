use qapi::caps::CapError;
use qapi::syscall::ops::vmtable::{VMLinkOp, VMMapOp, VMSetAttr, VMUnlinkOp, VMUnmapOp};

use super::SyscallResp;
use crate::arch::mem::Frame;
use crate::arch::{ArchCaps, ArchSystem, System};
use crate::thread::Thread;

pub fn vm_link(opts: VMLinkOp) -> SyscallResp {
    let top_table = Thread::with_current(|thread| thread.get_cap(opts.top_table.cap()))
        .ok_or(CapError::CapNotFound)?;
    let top_table = top_table.as_arch_cap()?;

    let bottom_table = Thread::with_current(|thread| thread.get_cap(opts.bottom_table.cap()))
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
        .ok_or(CapError::CapNotFound)?;
    let table = table.as_arch_cap()?;
    <ArchSystem as System>::ArchCaps::vm_unlink(table, opts.offset)
}

pub fn vm_map(opts: VMMapOp) -> SyscallResp {
    // TODO: Verify that frame is in component's capability set
    let top_table = Thread::with_current(|thread| thread.get_cap(opts.table.cap()))
        .ok_or(CapError::CapNotFound)?;
    let top_table = top_table.as_arch_cap()?;
    let frame = Frame::try_from(opts.frame)?;

    <ArchSystem as System>::ArchCaps::vm_map(top_table, opts.offset, frame, opts.flags.into())
}

pub fn vm_unmap(opts: VMUnmapOp) -> SyscallResp {
    let table = Thread::with_current(|thread| thread.get_cap(opts.table.cap()))
        .ok_or(CapError::CapNotFound)?;
    let table = table.as_arch_cap()?;
    <ArchSystem as System>::ArchCaps::vm_unmap(table, opts.offset)
}

pub fn vm_set_attr(opts: VMSetAttr) -> SyscallResp {
    let table = Thread::with_current(|thread| thread.get_cap(opts.table.cap()))
        .ok_or(CapError::CapNotFound)?;
    let table = table.as_arch_cap()?;
    <ArchSystem as System>::ArchCaps::vm_set_attributes(table, opts.offset, opts.flags.into())
}
