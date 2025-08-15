use core::mem::MaybeUninit;

use cfg_if::cfg_if;
use derive_more::{Display, Error, TryFrom};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::{
    caps::CapError,
    syscall::{InitSyscallParams, SYSCALL_ARGS, UninitSyscallParams},
};

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, KnownLayout, Immutable, IntoBytes, FromBytes)]
pub struct Frame {
    base: u64,
}

#[derive(Debug, Clone, Copy, Error, Display)]
pub enum InvalidFrame {
    #[display("The frame base address is not aligned to a frame boundary")]
    Misaligned,
}

impl Frame {
    #[cfg(target_arch = "x86_64")]
    pub const SIZE: usize = 4096;

    pub const fn try_new(start_addr: u64) -> Result<Self, InvalidFrame> {
        if start_addr % Self::SIZE as u64 != 0 {
            return Err(InvalidFrame::Misaligned);
        }
        Ok(Self { base: start_addr })
    }

    pub const fn from_index(index: usize) -> Self {
        match Self::try_new((index as u64) * (Self::SIZE as u64)) {
            Ok(t) => t,
            Err(InvalidFrame::Misaligned) => panic!("Bad address: Misaligned"),
        }
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn index(&self) -> usize {
        (self.base / (Self::SIZE as u64)) as usize
    }
}

#[repr(usize)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, TryFrom)]
#[try_from(repr)]
pub enum RetypeKind {
    IntoUntyped = 0,
    IntoKernel = 1,
    IntoUser = 2,
}

pub struct RetypeOp {
    pub frame: u64,
    pub to: RetypeKind,
}

impl RetypeOp {
    pub fn into_args(self) -> UninitSyscallParams {
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
    pub fn try_from_args(args: &InitSyscallParams) -> Result<Self, CapError> {
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
