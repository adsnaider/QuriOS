use qapi::caps::{CapError, PositiveIsize};
use qapi::syscall::ops::notify::NotifyOp;

use super::SyscallResp;
use crate::arch::{ArchSystem, System};
use crate::thread::Thread;

pub fn notify(opts: NotifyOp, ctx: <ArchSystem as System>::IrqCtx) -> SyscallResp {
    let dispatcher = {
        let notification = Thread::with_current(|thread| thread.get_cap(opts.notify_cap.cap()))
            .ok_or(CapError::CapNotFound)?;
        let notification = notification.as_notification()?;
        notification.signal(ctx)?
    };
    if let Some(dispatcher) = dispatcher {
        dispatcher.dispatch();
    } else {
        Ok(PositiveIsize::zero())
    }
}
