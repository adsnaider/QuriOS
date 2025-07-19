use crate::arch::{ArchSystem, System};
use qapi::{
    caps::{CapError, PositiveIsize},
    syscall::{SyscallArgs, SyscallArgsInit},
};

use crate::thread::Thread;

pub fn syscall_handler(
    cap: usize,
    args: SyscallArgs<SyscallArgsInit>,
    ctx: <ArchSystem as System>::SyscallCtx,
) -> Result<PositiveIsize, CapError> {
    let cap = cap.try_into()?;
    // SAFETY: We are allowed to get a mutable reference at the start of the syscall. It will get dropped
    log::info!("Handling syscall: {cap} with args {args:?} {ctx:#?}");
    // SAFETY: We should always have a thread set on syscall handling
    let cap = Thread::current_cap(cap).ok_or(CapError::CapNotFound)?;
    cap.exercise(args)
}
