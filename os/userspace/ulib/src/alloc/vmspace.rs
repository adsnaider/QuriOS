#![allow(unused)]
use allocator_api2::alloc::Allocator;
use derive_more::{Display, Error, From};
use hashbrown::{DefaultHashBuilder, HashMap};
use qapi::caps::CapError;
use qapi::caps::vmtable::VMTableCap;
use qapi::mem::virt::{PageTableLevel, PageTableOffset};
use qapi::mem::{Frame, Page, PageFlags};

use super::allocman::Resources;
use super::cspace::CSpace;
use super::pmspace::PMSpace;

enum EntryPayload {
    Link(VMTableCap),
    Page((Frame, PageFlags)),
}

pub struct Addrspace {
    root: VMTableCap,
    entries: (),
}

#[derive(Debug, Display, Error, From)]
pub enum MapError {
    CapError(CapError),
    #[display("The given page is already mapped to {_0:?}")]
    AlreadyMapped(#[error(not(source))] (Frame, PageFlags)),
}

impl Addrspace {
    pub fn new(root: VMTableCap) -> Self {
        Self {
            root,
            entries: todo!(),
        }
    }

    /// # Safety
    ///
    /// Modifying the address space is intrinsically unsafe
    pub unsafe fn map_to<F: PMSpace, C: CSpace>(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PageFlags,
        parent_flags: PageFlags,
        falloc: &F,
        cap_allocator: &mut C,
    ) -> Result<(), MapError> {
        /*
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
        */
        todo!();
    }
}

pub enum MapToError {}
pub enum UnmapError {}
pub enum TranslateError {}

pub(super) trait VMSpace {
    fn map_to(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PageFlags,
        parent_flags: PageFlags,
        resources: &mut Resources,
    ) -> Result<(), MapToError>;

    fn unmap(&mut self, page: Page, flags: PageFlags) -> Result<Frame, UnmapError>;
    fn translate_page(&mut self, page: Page) -> Result<Option<Frame>, TranslateError>;
}

impl VMSpace for Addrspace {
    fn map_to(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PageFlags,
        parent_flags: PageFlags,
        resources: &mut Resources,
    ) -> Result<(), MapToError> {
        todo!()
    }

    fn unmap(&mut self, page: Page, flags: PageFlags) -> Result<Frame, UnmapError> {
        todo!()
    }

    fn translate_page(&mut self, page: Page) -> Result<Option<Frame>, TranslateError> {
        todo!()
    }
}
