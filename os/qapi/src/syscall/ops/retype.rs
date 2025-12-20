use core::mem::MaybeUninit;

use derive_more::TryFrom;

use crate::caps::CapError;
use crate::mem::Frame;
use crate::syscall::ops::{u64_to_usize_array, usize_array_to_u64};
use crate::syscall::{InitSyscallParams, SYSCALL_ARGS, SyscallRequest, UninitSyscallParams};

#[repr(usize)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, TryFrom)]
#[try_from(repr)]
pub enum RetypeKind {
    IntoUntyped = 0,
    IntoKernel = 1,
    IntoUser = 2,
}

#[derive(Debug, Copy, Clone)]
pub struct RetypeOp {
    pub frame: Frame,
    pub to: RetypeKind,
}

impl SyscallRequest for RetypeOp {
    fn into_args(self) -> UninitSyscallParams {
        let mut args = [MaybeUninit::uninit(); SYSCALL_ARGS];
        let frame = u64_to_usize_array(self.frame.base());
        log::debug!("Frame: {frame:?}");
        let mut next_idx = 0;
        for piece in frame {
            args[next_idx] = MaybeUninit::new(piece);
            next_idx += 1;
        }
        args[next_idx] = MaybeUninit::new(self.to as usize);
        args
    }
    fn try_from_args(args: &InitSyscallParams) -> Result<Self, CapError> {
        let (frame, next_idx) = usize_array_to_u64(args);
        let to = RetypeKind::try_from(args[next_idx]).map_err(|_| CapError::InvalidArg)?;
        Ok(Self {
            frame: Frame::try_new(frame)?,
            to,
        })
    }
}
