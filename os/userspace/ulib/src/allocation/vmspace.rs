#![cfg(target_arch = "x86_64")]
#![allow(unused)]

use alloc::boxed::Box;

use derive_more::{Display, Error, From};
use hashbrown::{DefaultHashBuilder, HashMap};
use qapi::caps::CapError;
use qapi::caps::vmtable::VMTableCap;
use qapi::mem::virt::{PageTableLevel, PageTableOffset};
use qapi::mem::vmtable::VMEntryFlags;
use qapi::mem::{Frame, Page, PageFlags};
use qapi::syscall::ops::introspect::IntrospectResult;

use super::allocman::Resources;
use super::cspace::CSpace;
use super::pmspace::PMSpace;
use crate::allocation::allocman::SharedResources;
use crate::allocation::cspace::CAllocError;
use crate::allocation::pmspace::FrameAllocError;
use crate::sysops::CapIdExt;

pub const ENTRIES: usize = 512;

#[derive(Debug)]
enum PageTableEntry {
    Link(PageTable),
    Page((Frame, PageFlags)),
    Nil,
}

#[derive(Debug, Default)]
enum ShadowTable {
    #[default]
    Unsynced,
    Shadowed {
        entries: Box<[PageTableEntry; ENTRIES], &'static SharedResources>,
        level: PageTableLevel,
    },
}

#[derive(Debug)]
pub struct PageTable {
    entries: ShadowTable,
    cap: VMTableCap,
}

pub struct Addrspace {
    root: PageTable,
    resources: &'static SharedResources,
}

#[derive(Debug, Display, Error, From)]
pub enum MapError {
    CapError(CapError),
    #[display("The given page is already mapped to {_0:?}")]
    AlreadyMapped(#[error(not(source))] (Frame, PageFlags)),
}

impl ShadowTable {}

impl PageTable {
    pub const fn new(cap: VMTableCap) -> Self {
        Self {
            entries: ShadowTable::Unsynced,
            cap,
        }
    }

    pub fn entry(&self, at: PageTableOffset) -> &PageTableEntry {
        todo!();
    }

    fn link_at(&self, idx: PageTableOffset, new_table: &PageTable) -> Result<(), CapError> {
        todo!()
    }

    fn map_to(&self, idx: PageTableOffset, frame: Frame, flags: PageFlags) -> Result<(), CapError> {
        todo!()
    }

    fn level(&self) -> PageTableLevel {
        todo!()
    }
}

impl Addrspace {
    pub const fn new(root: VMTableCap, resources: &'static SharedResources) -> Self {
        Self {
            root: PageTable::new(root),
            resources,
        }
    }
}

#[derive(Debug, Error, Display, From)]
pub enum MapToError {
    #[display("System out of memory while allocating page tables")]
    OutOfMemory(FrameAllocError),
    #[display("System out of capability nodes while allocating page tables")]
    OutOfCaps(CAllocError),
    #[display("The page is already mapped")]
    AlreadyMapped(#[error(not(source))] (Frame, PageFlags)),
}
pub enum UnmapError {}
pub enum TranslateError {}

pub(super) trait VMSpace {
    fn map_to(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PageFlags,
        parent_flags: PageFlags,
    ) -> Result<(), MapToError>;

    fn unmap(&mut self, page: Page, flags: PageFlags) -> Result<Frame, UnmapError>;
    fn translate_page(&mut self, page: Page) -> Result<Option<Frame>, TranslateError>;
}

#[derive(Debug, Error, From, Display)]
enum GetTableError {
    CapError(CapError),
    OutOfMemory(FrameAllocError),
    OutOfCaps(CAllocError),
    FoundPage,
}

impl Addrspace {
    fn get_or_create_page_table<'a>(
        table: &'a PageTable,
        offset: PageTableOffset,
        flags: PageFlags,
        resources: &SharedResources,
    ) -> Result<&'a PageTable, GetTableError> {
        match table.entry(offset) {
            PageTableEntry::Link(page_table) => Ok(page_table),
            PageTableEntry::Page(_) => Err(GetTableError::FoundPage),
            PageTableEntry::Nil => {
                let new_table: PageTable = {
                    let table_frame = resources.borrow_mut().steal_frame()?;
                    let vmtable = resources
                        .borrow_mut()
                        .steal_cap()?
                        .make_vmtable(table_frame, table.level().as_u8() - 1)
                        .expect("Error creating VM Table");
                    PageTable::new(vmtable)
                };
                table
                    .link_at(offset, &new_table)
                    .expect("Error linking page tables");
                let PageTableEntry::Link(new_table) = table.entry(offset) else {
                    unreachable!();
                };
                Ok(new_table)
            }
        }
    }

    fn map_to_frame<'a>(
        table: &'a PageTable,
        offset: PageTableOffset,
        frame: Frame,
        flags: PageFlags,
    ) -> Result<(), MapToError> {
        assert!(table.level().is_bottom());
        match table.entry(offset) {
            PageTableEntry::Link(page_table) => {
                panic!("Unexpected bottom page table links to another table")
            }
            PageTableEntry::Page((frame, flags)) => {
                Err(MapToError::AlreadyMapped((*frame, *flags)))
            }
            PageTableEntry::Nil => {
                table
                    .map_to(offset, frame, flags)
                    .expect("Error linking page tables");
                Ok(())
            }
        }
    }
}

impl From<GetTableError> for MapToError {
    fn from(value: GetTableError) -> Self {
        match value {
            GetTableError::CapError(cap_error) => {
                panic!("Unexpected failure during syscall: {cap_error}")
            }
            GetTableError::OutOfMemory(frame_alloc_error) => Self::OutOfMemory(frame_alloc_error),
            GetTableError::OutOfCaps(calloc_error) => Self::OutOfCaps(calloc_error),
            GetTableError::FoundPage => panic!("Mismatched found page when expected page table"),
        }
    }
}

impl VMSpace for Addrspace {
    fn map_to(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PageFlags,
        parent_flags: PageFlags,
    ) -> Result<(), MapToError> {
        let table = Self::get_or_create_page_table(
            &self.root,
            page.p4_index(),
            parent_flags,
            &self.resources,
        )?;
        let table =
            Self::get_or_create_page_table(table, page.p3_index(), parent_flags, &self.resources)?;
        let table =
            Self::get_or_create_page_table(table, page.p2_index(), parent_flags, &self.resources)?;
        Self::map_to_frame(table, page.p1_index(), frame, flags)?;
        Ok(())
    }

    fn unmap(&mut self, page: Page, flags: PageFlags) -> Result<Frame, UnmapError> {
        todo!()
    }

    fn translate_page(&mut self, page: Page) -> Result<Option<Frame>, TranslateError> {
        todo!()
    }
}
