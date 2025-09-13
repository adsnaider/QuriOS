use qapi::caps::CapError;
use qapi::syscall::ops::thread::DispatchOp;

use crate::arch::{ArchSystem, System};
use crate::thread::{SigWaitResult, Thread};

use super::SyscallResp;

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
    let action = Thread::with_current(|t| t.sig_wait(ctx))?;
    match action {
        SigWaitResult::Blocked(dispatch_token) => dispatch_token.dispatch(),
        SigWaitResult::Signalled(signals) => Ok(signals.into()),
    }
}
