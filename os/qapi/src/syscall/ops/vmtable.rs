use derive_more::{Display, Error, Into};
use qapi_macros::SyscallRequest;

use crate::{
    caps::{CapError, vmtable::VMTableCap},
    mem::{
        PageFlags,
        virt::{PageTableOffset, PageTableOffsetError},
    },
};

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

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct VMSetAttr {
    pub table: VMTableCap,
    pub offset: PaddedPageTableOffset,
    pub flags: PageFlags,
}
