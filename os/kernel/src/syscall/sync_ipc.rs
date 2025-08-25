use super::SyscallResp;
use crate::arch::{ArchSystem, System};
use crate::thread::Thread;
use qapi::caps::CapError;
use qapi::syscall::ops::sync_ipc::{SyncInvokeOp, SyncRetOp};

pub fn sync_invoke(opts: SyncInvokeOp, ctx: <ArchSystem as System>::IrqCtx) -> SyscallResp {
    let cap = Thread::with_current(|thread| thread.get_cap(opts.cap.cap()))
        .unwrap()
        .ok_or(CapError::CapNotFound)?;
    let call_ctx = cap.as_synccall()?.clone().create_invocation(&ctx);
    Thread::with_current(move |thread| {
        thread.invoke(call_ctx, ctx)?;
    })
}

pub fn sync_ret(opts: SyncRetOp, ctx: <ArchSystem as System>::IrqCtx) -> SyscallResp {
    Thread::with_current(|thread| thread.sync_ret(opts, ctx))?;
}
