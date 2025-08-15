use qapi::mem::Frame;

pub trait FrameAllocator {
    fn alloc(&self) -> Option<Frame>;
    fn dealloc(&self, frame: Frame);
}

pub mod bitmap_allocator;
