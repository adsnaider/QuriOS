use qapi::{
    caps::CapError,
    syscall::ops::sync_ipc::{SyncInvokeOp, SyncRetOp},
};

use crate::{
    arch::{exec::ExecState, ArchSystem, System},
    sync_call::CallAbi,
    thread::Thread,
};

use super::SyscallResp;

pub fn sync_invoke(
    opts: SyncInvokeOp,
    ctx: <<ArchSystem as System>::ExecState as ExecState>::RegCtx,
) -> SyscallResp {
    let cap = Thread::with_current(|thread| thread.get_cap(opts.cap.cap()))
        .unwrap()
        .ok_or(CapError::CapNotFound)?;
    let sync_call = cap.as_synccall()?;
    Thread::with_current(move |thread| {
        thread.sync_invoke::<CallAbi>(sync_call.clone(), ctx)?;
    })
}

pub fn sync_ret(opts: SyncRetOp) -> SyscallResp {
    Thread::with_current(|thread| thread.sync_ret(opts))?;
}
