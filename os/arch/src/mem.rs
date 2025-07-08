mod phys;
mod pmo;
mod virt;

use core::marker::PhantomData;

use bitflags::bitflags;

use derive_more::{Display, Error, From};
pub use phys::{Frame, PhysAddr};
pub use pmo::Pmo;
pub use virt::{Page, VirtAddr};

use crate::KernelObject;

#[derive(Debug, Error, Display)]
pub enum FrameAllocError {
    #[display("No more frames in the system")]
    OutOfMemory,
}

pub trait FrameAllocator {
    fn alloc_kernel_frame(&mut self) -> Result<Frame, FrameAllocError>;
}

#[derive(Debug, Error, Display, From)]
pub enum MapPageError {
    #[from]
    AllocError(FrameAllocError),
    #[display("Tried to map over a huge entry")]
    HugeParentEntry,
    #[display("Tried to map an already mapped entry")]
    AlreadyMapped(#[error(not(source))] Frame),
}

pub trait Addrspace: KernelObject {
    /// Maps a page to the given frame for the provided addrspace.
    ///
    /// # Safety
    ///
    /// Making modifications to the addrspace may result in UB. This can be avoided
    /// by not modifying any higher-half (kernel-owned) memory.
    unsafe fn map_page<A: FrameAllocator>(
        &self,
        page: Page,
        frame: Frame,
        flags: PageFlags,
        parent_flags: PageFlags,
        alloc: &mut A,
    ) -> Result<Flusher<Self>, MapPageError>;

    // TODO: Support huge pages as well

    /// Flushes the given page, forcing the addrspace changes to go into effect
    fn flush(page: Page);

    fn activate(&self);
}

#[must_use]
pub struct Flusher<A: ?Sized>(Page, PhantomData<A>);

impl<A: Addrspace> Flusher<A> {
    pub fn new(page: Page) -> Self {
        Self(page, PhantomData)
    }

    pub fn flush(&self) {
        A::flush(self.0);
    }

    pub fn ignore(self) {}
}

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct PageFlags: u64 {
        const PRESENT = 1;
        const READABLE = 1 << 1;
        const WRITABLE = 1 << 2;
        const EXECUTABLE = 1 << 3;
        const USER_ACCESSIBLE = 1 << 4;
    }
}

impl PageFlags {
    pub fn readable(&self) -> bool {
        self.contains(PageFlags::READABLE)
    }

    pub fn writeable(&self) -> bool {
        self.contains(PageFlags::WRITABLE)
    }

    pub fn executable(&self) -> bool {
        self.contains(PageFlags::EXECUTABLE)
    }

    pub fn present(&self) -> bool {
        self.contains(PageFlags::PRESENT)
    }

    pub fn user_accessible(&self) -> bool {
        self.contains(PageFlags::USER_ACCESSIBLE)
    }
}

impl From<loader::MemFlags> for PageFlags {
    fn from(rwx: loader::MemFlags) -> Self {
        let mut pflags = PageFlags::PRESENT | PageFlags::USER_ACCESSIBLE;
        if rwx.readable() {
            pflags |= PageFlags::WRITABLE;
        }
        if rwx.writeable() {
            pflags |= PageFlags::WRITABLE;
        }
        if rwx.executable() {
            pflags |= PageFlags::EXECUTABLE;
        }
        pflags
    }
}
