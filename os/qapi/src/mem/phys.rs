use derive_more::{Debug, Display, Error};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::caps::CapError;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, KnownLayout, Immutable, IntoBytes, FromBytes)]
#[debug("Frame({:#X})", self.base)]
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

impl From<InvalidFrame> for CapError {
    fn from(_: InvalidFrame) -> Self {
        Self::FrameNotCanonical
    }
}
