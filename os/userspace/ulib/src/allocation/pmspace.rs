pub mod bitmap_allocator;

use derive_more::{Display, Error};
use qapi::mem::Frame;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Error, Display)]
pub enum FrameAllocError {
    #[display("This allocator is out of physical memory frames to give out")]
    OutOfFrames,
}

pub(super) trait PMSpace {
    fn alloc_frame(&mut self) -> Result<Frame, FrameAllocError>;
    unsafe fn dealloc(&mut self, frame: Frame);
}
