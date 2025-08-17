use derive_more::{Display, Error};

use crate::arch::mem::{Frame, FrameAllocError, FrameAllocator, PhysAddr};
use crate::retyping::{AsTypeError, FrameExt, KernelFrame, UserFrame};

#[derive(Debug)]
pub struct BumpFrameAllocator {
    index: u64,
}

#[derive(Debug, Copy, Clone, Error, Display)]
#[display("Out of system memory")]
pub struct OutOfMemory;

impl Default for BumpFrameAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl BumpFrameAllocator {
    pub fn new() -> Self {
        Self { index: 0 }
    }

    pub fn alloc_user_frame(&mut self) -> Result<UserFrame, OutOfMemory> {
        loop {
            if let Ok(frame) = self.alloc_untyped_frame()?.try_into_user() {
                return Ok(frame);
            }
        }
    }

    pub fn alloc_untyped_frame(&mut self) -> Result<Frame, OutOfMemory> {
        loop {
            let frame = self.next_available();
            self.index += 1;
            log::trace!("Trying to allocate untyped frame: {frame:?}");
            match frame.try_as_untyped() {
                Ok(frame) => return Ok(frame),
                Err(AsTypeError::OutOfBounds(_)) => return Err(OutOfMemory),
                Err(e) => log::trace!("Err: {e:?}"),
            }
        }
    }

    pub fn alloc_kernel_frame(&mut self) -> Result<KernelFrame, OutOfMemory> {
        loop {
            if let Ok(frame) = self.alloc_untyped_frame()?.try_into_kernel() {
                return Ok(frame);
            }
        }
    }

    pub fn next_available(&self) -> Frame {
        Frame::from_start_address(PhysAddr::new(Frame::SIZE * self.index))
    }
}

impl FrameAllocator for BumpFrameAllocator {
    fn alloc_kernel_frame(&mut self) -> Result<Frame, FrameAllocError> {
        Ok(self
            .alloc_kernel_frame()
            .map_err(|_| FrameAllocError::OutOfMemory)?
            .into_raw())
    }
}
