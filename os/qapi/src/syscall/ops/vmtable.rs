use core::mem::MaybeUninit;

use derive_more::{Display, Error, Into};
use qapi_macros::SyscallRequest;

use super::{u64_to_usize_array, usize_array_to_u64};
use crate::caps::CapError;
use crate::caps::vmtable::VMTableCap;
use crate::mem::virt::{PageTableOffset, PageTableOffsetError};
use crate::mem::{Frame, PageFlags};
use crate::syscall::{SYSCALL_ARGS, SyscallRequest};

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Into)]
pub struct PaddedPageTableOffset(usize);

#[derive(Debug, Clone, Copy, Error, Display)]
pub enum BadVMOffset {
    #[display("The offset exceeds the bit-width of the arch-specific page table")]
    OutOfBounds,
}

impl TryFrom<usize> for PaddedPageTableOffset {
    type Error = BadVMOffset;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        let off = PageTableOffset::try_from(value)?;
        Ok(Self(off.into()))
    }
}

impl From<BadVMOffset> for CapError {
    fn from(value: BadVMOffset) -> Self {
        match value {
            BadVMOffset::OutOfBounds => Self::InvalidArg,
        }
    }
}

impl From<PageTableOffsetError> for BadVMOffset {
    fn from(value: PageTableOffsetError) -> Self {
        match value {
            PageTableOffsetError::OutOfBounds => Self::OutOfBounds,
        }
    }
}

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct VMLinkOp {
    pub top_table: VMTableCap,
    pub offset: PaddedPageTableOffset,
    pub bottom_table: VMTableCap,
    pub flags: PageFlags,
}

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct VMUnlinkOp {
    pub table: VMTableCap,
    pub offset: PaddedPageTableOffset,
}

#[derive(Debug, Copy, Clone)]
pub struct VMMapOp {
    pub table: VMTableCap,
    pub offset: PaddedPageTableOffset,
    pub frame: Frame,
    pub flags: PageFlags,
}

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct VMUnmapOp {
    pub table: VMTableCap,
    pub offset: PaddedPageTableOffset,
}

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct VMSetAttr {
    pub table: VMTableCap,
    pub offset: PaddedPageTableOffset,
    pub flags: PageFlags,
}

impl SyscallRequest for VMMapOp {
    fn into_args(self) -> crate::syscall::UninitSyscallParams {
        let mut args = [MaybeUninit::uninit(); SYSCALL_ARGS];
        let mut next_idx = 0;
        args[next_idx] = MaybeUninit::new(self.table.into());
        next_idx += 1;
        args[next_idx] = MaybeUninit::new(self.offset.into());
        next_idx += 1;
        let frame_pieces = u64_to_usize_array(self.frame.base());
        for piece in frame_pieces {
            args[next_idx] = MaybeUninit::new(piece);
            next_idx += 1;
        }
        args[next_idx] = MaybeUninit::new(self.flags.into());
        args
    }

    fn try_from_args(args: &crate::syscall::InitSyscallParams) -> Result<Self, CapError> {
        let mut next_idx = 0;
        let top_table = args[next_idx].try_into()?;
        next_idx += 1;
        let offset = args[next_idx].try_into()?;
        next_idx += 1;
        let (frame, count) = usize_array_to_u64(args);
        next_idx += count;
        let flags = args[next_idx].try_into()?;
        Ok(Self {
            table: top_table,
            offset,
            frame: Frame::try_new(frame)?,
            flags,
        })
    }
}
