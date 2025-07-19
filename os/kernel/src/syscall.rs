use arch::{ArchSystem, System};
use qapi::{
    caps::{CapError, PositiveIsize},
    syscall::{SyscallArgs, SyscallArgsInit},
};
use tap::Tap;

use crate::thread::Thread;

pub fn syscall_handler(
    cap: usize,
    args: SyscallArgs<SyscallArgsInit>,
    ctx: <ArchSystem as System>::SyscallCtx,
) -> Result<PositiveIsize, CapError> {
    let cap = cap.try_into()?;
    log::info!("Handling syscall: {cap} with args {args:?} {ctx:#?}");
    let current = Thread::current();
    // SAFETY: We should always have a thread set on syscall handling
    let cap = unsafe {
        current
            .as_ref()
            .tap(|t| debug_assert!(t.is_some()))
            .unwrap_unchecked()
            .active_comp()
            .cap(cap)
    }
    .ok_or(CapError::CapNotFound)?;
    cap.exercise(args)
}
