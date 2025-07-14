use arch::System;
use qapi::{
    caps::{CapError, CapIndex, PositiveIsize},
    syscall::{SyscallArgs, SyscallArgsInit},
};

use crate::thread::Thread;

pub fn syscall_handler<S: System>(
    args: SyscallArgs<SyscallArgsInit>,
    ctx: S::SyscallCtx,
) -> Result<PositiveIsize, CapError> {
    let cap = CapIndex::try_from(args.cap())?;
    let args = args.args();
    log::info!("Handling syscall: {cap} with args {args:?} {ctx:#?}");
    let _comp = Thread::<S>::current().active_comp().cap(cap);
    todo!();
}
