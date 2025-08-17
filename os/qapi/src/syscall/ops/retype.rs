use core::mem::MaybeUninit;

use cfg_if::cfg_if;
use derive_more::TryFrom;

use crate::caps::CapError;
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
    pub frame: u64,
    pub to: RetypeKind,
}

impl SyscallRequest for RetypeOp {
    fn into_args(self) -> UninitSyscallParams {
        let mut args = [MaybeUninit::uninit(); SYSCALL_ARGS];
        cfg_if! {
            if #[cfg(target_pointer_width = "64")] {
                args[0] = MaybeUninit::new(self.frame as usize);
                let next_idx = 1;
            } else if #[cfg(target_pointer_width = "32")] {
                args[0] = MaybeUninit::new((self.frame & u32::MAX as u64) as usize);
                args[1] = MaybeUninit::new((self.frame >> 32 & u32::MAX as u64) as usize);
                let next_idx = 2;
            } else {
                const {
                    panic!("Unhandled pointer width");
                }
            }
        };
        args[next_idx] = MaybeUninit::new(self.to as usize);
        args
    }
    fn try_from_args(args: &InitSyscallParams) -> Result<Self, CapError> {
        cfg_if! {
            if #[cfg(target_pointer_width = "64")] {
                let frame = args[0] as u64;
                let next_idx = 1;
            } else if #[cfg(target_pointer_width = "32")] {
                let frame = args[0] as u64 + args[1] as u64 << 32;
                next_idx = 2;
            } else {
                const {
                    panic!("Unhandled pointer width");
                }
            }
        }
        let to = RetypeKind::try_from(args[next_idx]).map_err(|_| CapError::InvalidArg)?;
        Ok(Self { frame, to })
    }
}
