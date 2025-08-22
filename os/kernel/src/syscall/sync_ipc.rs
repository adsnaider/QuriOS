use qapi::{
    caps::CapError,
    syscall::ops::sync_ipc::{SyncInvokeOp, SyncRetOp},
};

use crate::{
    arch::{exec::ExecState, ArchSystem, System},
    thread::Thread,
};

use super::SyscallResp;

pub fn sync_invoke(
    opts: SyncInvokeOp,
    ctx: <<ArchSystem as System>::ExecState as ExecState>::RegCtx,
) -> SyscallResp {
    let cap = Thread::get_cap(opts.cap.cap()).ok_or(CapError::CapNotFound)?;
    let sync_call = cap.as_synccall()?;
    Thread::sync_invoke(sync_call.clone(), opts.args, ctx)?;
}

pub fn sync_ret(opts: SyncRetOp) -> SyscallResp {
    Thread::sync_ret(opts)?;
}
