use arch::mem::{Frame, PhysAddr};
use limine::memory_map::{Entry, EntryType};

use super::MemoryMap;

pub struct BumpAllocator {
    memory_map: MemoryMap,
    index: usize,
}

#[allow(unused)]
impl BumpAllocator {
    pub fn new(memory_map: MemoryMap) -> Self {
        Self {
            memory_map,
            index: 0,
        }
    }

    pub fn alloc_frame(&mut self) -> Option<Frame> {
        let frame = loop {
            let entry = self.memory_map.get_mut(self.index)?;
            assert!(entry.length % Frame::SIZE == 0);
            if entry.entry_type == EntryType::USABLE && entry.length > 0 {
                let start_address = entry.base;
                entry.base += Frame::SIZE;
                entry.length -= Frame::SIZE;
                let start_address = PhysAddr::new(start_address);
                break Frame::from_start_address(start_address);
            }
            self.index += 1;
        };
        Some(frame)
    }

    pub fn alloc_frames(&mut self, count: usize) -> Option<PhysAddr> {
        let requested_length = count as u64 * Frame::SIZE;
        let start_address = loop {
            let entry = self.memory_map.get_mut(self.index)?;
            assert!(entry.length % Frame::SIZE == 0);
            if entry.entry_type == EntryType::USABLE && entry.length >= requested_length {
                let start_address = entry.base;
                entry.base += requested_length;
                entry.length -= requested_length;
                break PhysAddr::new(start_address);
            }
            self.index += 1;
        };
        Some(start_address)
    }

    pub fn into_memory_map(self) -> &'static mut [&'static mut Entry] {
        self.memory_map
    }

    pub fn memory_map(&mut self) -> &mut [&'static mut Entry] {
        self.memory_map
    }
}
