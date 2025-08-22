use qapi::caps::{CapError, PositiveIsize};
use qapi::syscall::ops::thread::DispatchOp;

use crate::arch::exec::ExecState;
use crate::arch::{ArchSystem, System};
use crate::thread::Thread;

pub fn dispatch(
    DispatchOp { thread_cap }: DispatchOp,
    ctx: <<ArchSystem as System>::ExecState as ExecState>::RegCtx,
) -> Result<PositiveIsize, CapError> {
    let thread = Thread::with_current(|thread| thread.get_cap(thread_cap.cap()))
        .unwrap()
        .ok_or(CapError::CapNotFound)?;
    let thread = thread.as_thread()?;

    Thread::dispatch(thread.clone(), Some(ctx)).map_err(|_| CapError::ThreadBoundToOtherCore)?;
}
