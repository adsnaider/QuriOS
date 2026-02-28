use derive_more::{Display, Error};
use qapi::init::{RetypeEntry, RetypeState};
use qapi::mem::Frame;
use qapi::syscall::ops::retype::RetypeKind;

use super::{FrameAllocError, PMSpace};
use crate::sysops::FrameExt as _;

pub struct BitmapAllocator {
    memory_map: MemoryMap<'static>,
}

impl BitmapAllocator {
    pub fn new(memory_map: &'static [RetypeEntry]) -> Self {
        Self {
            memory_map: MemoryMap::new(memory_map),
        }
    }
}

#[derive(Debug, Clone, Error, Display)]
pub enum PmmError {}

struct MemoryMap<'a> {
    inner: &'a [RetypeEntry],
}

impl<'a> MemoryMap<'a> {
    pub const fn new(memory_map: &'a [RetypeEntry]) -> Self {
        Self { inner: memory_map }
    }

    pub fn iter(&self) -> MMapIter<'a> {
        MMapIter {
            index: 0,
            map: self.inner,
        }
    }

    pub const fn len(&self) -> usize {
        self.inner.len()
    }
}

struct MMapIter<'a> {
    index: usize,
    map: &'a [RetypeEntry],
}

impl<'a> Iterator for MMapIter<'a> {
    type Item = (Frame, &'a RetypeEntry);

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.map.get(self.index)?;
        let frame = Frame::from_index(self.index);
        self.index += 1;
        Some((frame, entry))
    }
}

impl BitmapAllocator {
    pub fn alloc(&mut self) -> Result<Frame, FrameAllocError> {
        self.memory_map
            .iter()
            .find(|(_, e)| e.state() == RetypeState::Untyped)
            .map(|(frame, _)| frame)
            .ok_or(FrameAllocError::OutOfFrames)
    }
}

impl PMSpace for BitmapAllocator {
    fn alloc_frame(&mut self) -> Result<Frame, super::FrameAllocError> {
        self.alloc()
    }

    unsafe fn dealloc(&mut self, frame: Frame) {
        frame.retype(RetypeKind::IntoUntyped).unwrap();
    }
}
