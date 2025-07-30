#[cfg(feature = "userspace")]
pub mod ulib;

use core::mem::MaybeUninit;

use crate::syscall::{InitSyscallParams, SYSCALL_ARGS, UninitSyscallParams};

use super::{CapError, CapId};

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct Thread {
    cap: CapId,
}

impl Thread {
    pub const fn new(cap: CapId) -> Self {
        Self { cap }
    }
}

pub struct DispatchOp {
    pub thread_cap: CapId,
}

impl DispatchOp {
    pub fn into_args(self) -> UninitSyscallParams {
        let mut args = [MaybeUninit::uninit(); SYSCALL_ARGS];
        args[0] = MaybeUninit::new(self.thread_cap.into());
        args
    }
    pub fn try_from_args(args: &InitSyscallParams) -> Result<Self, CapError> {
        Ok(Self {
            thread_cap: CapId::try_from(args[0])?,
        })
    }
}
