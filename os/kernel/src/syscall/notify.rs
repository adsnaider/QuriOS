use qapi::{
    caps::{CapError, PositiveIsize},
    syscall::ops::notify::NotifyOp,
};

use crate::{
    arch::{ArchSystem, System},
    thread::Thread,
};

use super::SyscallResp;

pub fn notify(opts: NotifyOp, ctx: <ArchSystem as System>::IrqCtx) -> SyscallResp {
    let notification = Thread::with_current(|thread| thread.get_cap(opts.notify_cap.cap()))
        .unwrap()
        .ok_or(CapError::CapNotFound)?;
    let notification = notification.as_notification()?;
    notification.signal(ctx)?;
    Ok(PositiveIsize::zero())
}
