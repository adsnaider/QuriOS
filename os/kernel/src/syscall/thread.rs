use qapi::caps::CapError;
use qapi::syscall::ops::thread::DispatchOp;

use super::SyscallResp;
use crate::arch::{ArchSystem, System};
use crate::thread::{SigWaitResult, Thread};
use crate::util::OptionExt as _;

pub fn dispatch(
    DispatchOp { thread_cap }: DispatchOp,
    ctx: <ArchSystem as System>::IrqCtx,
) -> SyscallResp {
    let dispatcher = {
        let thread = Thread::with_current(|thread| thread.get_cap(thread_cap.cap()))
            .ok_or(CapError::CapNotFound)?;
        let thread = thread.as_thread()?;

        Thread::dispatch(thread.clone(), ctx)?
    };
    dispatcher.dispatch();
}

pub fn thread_sig_wait(ctx: <ArchSystem as System>::IrqCtx) -> SyscallResp {
    // SAFETY: Should always have a current thread on an Irq context
    let thread = unsafe { Thread::get_current().unwrap_debug() };
    match Thread::sig_wait(thread, ctx)? {
        SigWaitResult::Blocked(dispatch_token) => dispatch_token.dispatch(),
        SigWaitResult::Signalled(signals) => Ok(signals.into()),
        SigWaitResult::Redispatch(dispatch_token) => dispatch_token.dispatch(),
    }
}
