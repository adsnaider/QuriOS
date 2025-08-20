use allocator_api2::alloc::Allocator;
use derive_more::{Display, Error, From};
use hashbrown::{DefaultHashBuilder, HashMap};
use qapi::caps::CapError;
use qapi::caps::vmtable::VMTableCap;
use qapi::mem::virt::{PageTableLevel, PageTableOffset};
use qapi::mem::{Frame, Page, PageFlags};

use super::caps::CapAlloc;
use super::phys::FrameAllocator;

enum EntryPayload {
    Link(VMTableCap),
    Page((Frame, PageFlags)),
}

pub struct Addrspace<A: Allocator> {
    root: VMTableCap,
    entries: HashMap<(VMTableCap, PageTableOffset), EntryPayload, DefaultHashBuilder, A>,
}

#[derive(Debug, Display, Error, From)]
pub enum MapError {
    CapError(CapError),
    #[display("The given page is already mapped to {_0:?}")]
    AlreadyMapped(#[error(not(source))] (Frame, PageFlags)),
}

impl<A: Allocator> Addrspace<A> {
    pub fn new(root: VMTableCap, allocator: A) -> Self {
        Self {
            root,
            entries: HashMap::new_in(allocator),
        }
    }

    pub unsafe fn map_to<F: FrameAllocator, C: CapAlloc>(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PageFlags,
        parent_flags: PageFlags,
        falloc: &F,
        cap_allocator: &mut C,
    ) -> Result<(), MapError> {
        let mut level = Some(PageTableLevel::top());
        let mut table = self.root;
        while let Some(current_level) = level {
            level = current_level.lower();
            let index = page.page_table_index(current_level);
            match self.entries.get(&(table, index)) {
                Some(&EntryPayload::Link(link)) => table = link,
                Some(&EntryPayload::Page((frame, flags))) => {
                    return Err(MapError::AlreadyMapped((frame, flags)));
                }
                None if current_level.is_bottom() => {
                    todo!();
                }
                None => {}
            }
        }
        todo!();
    }

    pub fn allocator(&self) -> &A {
        self.entries.allocator()
    }
}
