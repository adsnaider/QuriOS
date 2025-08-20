use core::mem::MaybeUninit;

use crate::{
    caps::{CapError, PositiveIsize, sync_ipc::SyncInvokeCap},
    syscall::{InitSyscallParams, SYSCALL_ARGS, SyscallRequest, UninitSyscallParams},
};

pub const SYNC_CALL_ARGS: usize = SYSCALL_ARGS - 1;
pub const SYNC_CALL_RETS: usize = 2;

pub type SyncCallFun = extern "C" fn(a: usize, b: usize, c: usize, d: usize) -> PositiveIsize;

#[derive(Debug, Copy, Clone)]
pub struct SyncInvokeOp {
    pub cap: SyncInvokeCap,
    pub args: [MaybeUninit<usize>; SYNC_CALL_ARGS],
}

impl SyscallRequest for SyncInvokeOp {
    fn into_args(self) -> UninitSyscallParams {
        let mut args = [MaybeUninit::uninit(); SYSCALL_ARGS];
        args[0] = MaybeUninit::new(self.cap.into());
        for (i, callarg) in self.args.iter().copied().enumerate() {
            args[i + 1] = callarg;
        }
        args
    }

    fn try_from_args(args: &InitSyscallParams) -> Result<Self, CapError> {
        let cap = args[0].try_into()?;
        let mut callargs = [MaybeUninit::uninit(); SYNC_CALL_ARGS];
        for i in 1..SYSCALL_ARGS {
            callargs[i - 1] = MaybeUninit::new(args[i]);
        }
        Ok(Self {
            cap,
            args: callargs,
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SyncRetOp {
    pub resp: PositiveIsize,
}

impl SyscallRequest for SyncRetOp {
    fn into_args(self) -> UninitSyscallParams {
        let mut args = [MaybeUninit::uninit(); SYSCALL_ARGS];
        args[0] = MaybeUninit::new(self.resp.into());
        args
    }

    fn try_from_args(args: &InitSyscallParams) -> Result<Self, CapError> {
        Ok(Self {
            resp: args[0].try_into()?,
        })
    }
}
