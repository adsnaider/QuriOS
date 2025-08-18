pub mod phys;
pub mod virt;
pub mod vmtable;

use bitflags::bitflags;
use derive_more::{Display, Error};
pub use phys::Frame;
pub use virt::Page;

use crate::caps::CapError;

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct PageFlags: usize {
        const PRESENT = 1;
        const READABLE = 1 << 1;
        const WRITABLE = 1 << 2;
        const EXECUTABLE = 1 << 3;
    }
}

#[derive(Debug, Clone, Copy, Display, Error)]
#[display("Unexpected bits set in page flags attributes")]
pub struct InvalidPageFlagsBits;

impl From<InvalidPageFlagsBits> for CapError {
    fn from(_: InvalidPageFlagsBits) -> Self {
        Self::InvalidArg
    }
}

impl TryFrom<usize> for PageFlags {
    type Error = InvalidPageFlagsBits;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::from_bits(value).ok_or(InvalidPageFlagsBits)
    }
}

impl From<PageFlags> for usize {
    fn from(value: PageFlags) -> Self {
        value.bits()
    }
}
