use derive_more::{Display, Error};
use qapi::{
    init::{RetypeEntry, RetypeState},
    mem::{Frame, phys::RetypeKind},
};

use super::FrameAllocator;

pub struct BitmapAllocator {
    // addrspace: PageTableCap,
    // bitmap: &'static BitSlice,
    memory_map: MemoryMap<'static>,
}

impl BitmapAllocator {
    pub fn new(
        // addrspace: PageTableCap,
        memory_map: &'static [RetypeEntry],
        // heap_start: usize,
    ) -> Self {
        Self {
            memory_map: MemoryMap::new(memory_map),
        }
        /*
        let frames_required = {
            let bit_count = memory_map.len();
            bit_count.div_ceil(Frame::SIZE * 8)
        };
        let memory_map = MemoryMap::new(memory_map);

        let heap_start = {
            // We can ignore all existing page tables if we just adjust the start of the heap to a
            // known unused L3 page table
            #[cfg(target_arch = "x86_64")]
            const TOP_LEVEL_MULTIPLE: usize = 0x0000_8000_0000_0000;
            heap_start.next_multiple_of(TOP_LEVEL_MULTIPLE)
        };
        let mut heap_end: usize = heap_start;

        memory_map
            .iter()
            .filter(|(_, e)| e.state() == RetypeState::Untyped)
            .take_while(|_| frames_required > 0)
            .for_each(|(frame, _)| {
                todo!();
            });
        todo!();

        // Self { memory_map }
        // */
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

impl FrameAllocator for BitmapAllocator {
    fn alloc(&self) -> Option<Frame> {
        let frame = self
            .memory_map
            .iter()
            .find(|(_, e)| e.state() == RetypeState::Untyped)
            .map(|(frame, _)| frame)?;
        frame.retype(RetypeKind::IntoUser).ok()?;
        Some(frame)
    }

    fn dealloc(&self, frame: Frame) {
        frame.retype(RetypeKind::IntoUntyped).unwrap();
    }
}

impl<F: FrameAllocator> FrameAllocator for &F {
    fn alloc(&self) -> Option<Frame> {
        (*self).alloc()
    }

    fn dealloc(&self, frame: Frame) {
        (*self).dealloc(frame)
    }
}
