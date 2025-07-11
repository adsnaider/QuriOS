#![allow(unused)]
use core::sync::atomic::{AtomicU64, Ordering};

use x86_64::{
    instructions::tlb,
    registers::control::Cr3,
    structures::paging::{PageTableFlags, PhysFrame},
};

use crate::{
    KernelObject,
    mem::{
        Addrspace, Flusher, Frame, FrameAllocator, MapPageError, Page, PageFlags, PhysAddr, Pmo,
        VirtAddr,
    },
};

pub struct X64Addrspace {
    l4_frame: Frame,
    pmo: Pmo,
}

impl X64Addrspace {
    /// Constructs a new addrspace with all the kernel pages mapped into it
    ///
    /// # Safety
    ///
    /// The provided l4_frame must be guranteed to not be used outside for anything else.
    /// It's reasonable to have multiple threads with a reference to the same addrspace but
    /// the frame must not be reused/recycled until all addrspaces are dropped and the current
    /// active addrspace is not one of them.
    pub unsafe fn new_with_kernel_entries(l4_frame: Frame, pmo: Pmo) -> Self {
        let current = Self::current(pmo);
        let mut new_l4 = AnyPageTable::new();
        for i in 256..512 {
            let offset = PageTableOffset::new(i).unwrap();
            if let Some((frame, flags)) = current.l4_table().get(offset).get() {
                log::debug!("Mapping kernel map: {frame:?}, {flags:?}");
                // SAFETY: Mapping kernel pages from the original addrspace is reasonable
                unsafe {
                    new_l4.map_mut(offset, frame, flags);
                }
            }
        }
        // SAFETY: We have exclusive access to the frame as we are initializaing it.
        unsafe { core::ptr::write(pmo.phys_to_virt(l4_frame.addr()).as_mut_ptr(), new_l4) };
        Self { l4_frame, pmo }
    }

    fn l4_table(&self) -> &AnyPageTable {
        let addr = self.pmo.phys_to_virt(self.l4_frame.addr()).as_ptr();
        // SAFETY: As long as this addrspace is valid, the frame is guaranteed to not be freed up.
        unsafe { &*addr }
    }

    pub fn current(pmo: Pmo) -> Self {
        let frame = Self::current_raw();
        Self {
            pmo,
            l4_frame: frame,
        }
    }

    pub fn current_raw() -> Frame {
        let (frame, _flags) = Cr3::read();
        Frame::from_start_address(PhysAddr::new(frame.start_address().as_u64()))
    }
}

impl From<Frame> for PhysFrame {
    fn from(value: Frame) -> Self {
        // SAFETY: Transparent representation
        unsafe { core::mem::transmute(value) }
    }
}

impl From<VirtAddr> for x86_64::VirtAddr {
    fn from(value: VirtAddr) -> Self {
        // SAFETY: Transparent representation
        unsafe { core::mem::transmute(value) }
    }
}

impl From<PageFlags> for PageTableFlags {
    fn from(value: PageFlags) -> Self {
        let mut flags = PageTableFlags::empty();
        if !value.readable() {
            log::trace!("(non)-readable bit is invalid on x86-64");
        }
        if value.writeable() {
            flags |= PageTableFlags::WRITABLE;
        }
        if !value.executable() {
            flags |= PageTableFlags::NO_EXECUTE;
        }
        if value.present() {
            flags |= PageTableFlags::PRESENT;
        }
        if value.user_accessible() {
            flags |= PageTableFlags::USER_ACCESSIBLE;
        }
        flags
    }
}

impl KernelObject for X64Addrspace {
    fn into_frame(self) -> Frame {
        self.l4_frame
    }
}

impl Addrspace for X64Addrspace {
    unsafe fn map_page<A: FrameAllocator>(
        &self,
        page: Page,
        frame: Frame,
        flags: PageFlags,
        parent_flags: PageFlags,
        alloc: &mut A,
    ) -> Result<Flusher<Self>, MapPageError> {
        let flags = flags.into();
        let parent_flags: PageTableFlags = parent_flags.into();
        let mut level = Some(PageTableLevel::top());
        let mut table = self.l4_table();
        let addr = page.base();
        while let Some(current_level) = level {
            level = current_level.lower();
            let offset = addr.page_table_index(current_level);
            let entry = table.get(offset);
            match entry.get() {
                Some((frame, flags)) => {
                    if current_level.level() == 1 {
                        return Err(MapPageError::AlreadyMapped(frame));
                    }
                    if flags.contains(PageTableFlags::HUGE_PAGE) {
                        return Err(MapPageError::HugeParentEntry);
                    }
                    // SAFETY: All mapped entries that contain a frame will have a valid page table frame
                    table = unsafe { &*self.pmo.phys_to_virt(frame.base()).as_ptr() };
                }
                None => {
                    if current_level.is_bottom() {
                        // SAFETY: Function precondition
                        unsafe { entry.set(frame, flags) };
                    } else {
                        let frame = alloc.alloc_kernel_frame()?;
                        let addr: *mut AnyPageTable =
                            self.pmo.phys_to_virt(frame.base()).as_mut_ptr();
                        // SAFETY: The address is valid and we have ownership (as it was just allocated)
                        unsafe { addr.write(AnyPageTable::new()) };
                        // SAFETY: addr contains a valid AnyPageTable.
                        table = unsafe { &*addr };
                        // SAFETY: It's okay to use the unused frrame for a new page table.
                        unsafe { entry.set(frame, parent_flags | PageTableFlags::PRESENT) };
                    }
                }
            }
        }
        Ok(Flusher::new(page))
    }

    fn flush(page: Page) {
        tlb::flush(page.base().into());
    }

    fn activate(&self) {
        let (frame, flags) = Cr3::read();
        let this_frame = self.l4_frame.into();
        if frame != this_frame {
            // SAFETY: Precondition for creating the Addrspace is that it remains valid.
            unsafe { Cr3::write(this_frame, flags) };
        }
    }
}

#[extend::ext]
impl VirtAddr {
    /// Returns the 9-bit level 1 page table index.
    #[inline]
    fn p1_index(self) -> PageTableOffset {
        PageTableOffset::new_truncate((self.0 >> 12) as u16)
    }

    /// Returns the 9-bit level 2 page table index.
    #[inline]
    fn p2_index(self) -> PageTableOffset {
        PageTableOffset::new_truncate((self.0 >> 12 >> 9) as u16)
    }

    /// Returns the 9-bit level 3 page table index.
    #[inline]
    fn p3_index(self) -> PageTableOffset {
        PageTableOffset::new_truncate((self.0 >> 12 >> 9 >> 9) as u16)
    }

    /// Returns the 9-bit level 4 page table index.
    #[inline]
    fn p4_index(self) -> PageTableOffset {
        PageTableOffset::new_truncate((self.0 >> 12 >> 9 >> 9 >> 9) as u16)
    }

    /// Returns the 9-bit level page table index.
    #[inline]
    fn page_table_index(self, level: PageTableLevel) -> PageTableOffset {
        PageTableOffset::new_truncate((self.0 >> 12 >> ((level.level() - 1) * 9)) as u16)
    }
}

#[repr(C, align(4096))]
#[derive(Debug)]
pub struct AnyPageTable([PageTableEntry; 512]);

impl Default for AnyPageTable {
    fn default() -> Self {
        Self::new()
    }
}

impl AnyPageTable {
    pub const fn new() -> Self {
        // SAFETY: This is correct for a page table
        unsafe { core::mem::zeroed() }
    }

    pub fn get(&self, offset: PageTableOffset) -> &PageTableEntry {
        // SAFETY: Offset is within [0, 512)
        unsafe { self.0.get_unchecked(offset.0 as usize) }
    }

    pub fn get_mut(&mut self, offset: PageTableOffset) -> &mut PageTableEntry {
        // SAFETY: Offset is within [0, 512)
        unsafe { self.0.get_unchecked_mut(offset.0 as usize) }
    }

    /// Atomically sets the frame and attributes on the page table offset provided
    ///
    /// # Safety
    ///
    /// This is one of those methods that fundamentally change memory and can cause undefined
    /// behaviour even when the usage is semantically reasonable.
    pub unsafe fn map(
        &self,
        offset: PageTableOffset,
        frame: Frame,
        attributes: PageTableFlags,
    ) -> Option<(Frame, PageTableFlags)> {
        // SAFETY: Precondition
        unsafe { self.get(offset).set(frame, attributes) }
    }

    pub unsafe fn map_mut(
        &mut self,
        offset: PageTableOffset,
        frame: Frame,
        attributes: PageTableFlags,
    ) -> Option<(Frame, PageTableFlags)> {
        // SAFETY: Precondition
        unsafe { self.get_mut(offset).set_mut(frame, attributes) }
    }

    /// Atomically sets the frame and attributes on the page table offset provided if none present
    ///
    /// # Safety
    ///
    /// This is one of those methods that fundamentally change memory and can cause undefined
    /// behaviour even when the usage is semantically reasonable.
    pub unsafe fn try_map(
        &self,
        offset: PageTableOffset,
        frame: Frame,
        attributes: PageTableFlags,
    ) -> Result<(), (Frame, PageTableFlags)> {
        // SAFETY: Precondition
        unsafe { self.get(offset).try_set(frame, attributes) }
    }

    /// Atomically unamps the entry (leaving it available for use again).
    ///
    /// # Safety
    ///
    /// This is one of those methods that fundamentally change memory and can cause undefined
    /// behaviour even when the usage is semantically reasonable.
    pub unsafe fn unmap(&self, offset: PageTableOffset) -> Option<(Frame, PageTableFlags)> {
        // SAFETY: Precondition
        unsafe { self.get(offset).reset() }
    }

    /// Atomically sets the flags on the provided entry.
    ///
    /// # Notes
    ///
    /// The operations themselves are atomic, however, there's no guarantee that another
    /// thread hasn't modified the frame.
    ///
    /// # Safety
    ///
    /// This is one of those methods that fundamentally change memory and can cause undefined
    /// behaviour even when the usage is semantically reasonable.
    pub unsafe fn set_flags(
        &self,
        offset: PageTableOffset,
        attributes: PageTableFlags,
    ) -> PageTableFlags {
        // SAFETY: Precondition
        unsafe { self.get(offset).set_flags(attributes) }
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct PageTableEntry(AtomicU64);

impl Default for PageTableEntry {
    fn default() -> Self {
        Self::new()
    }
}

impl PageTableEntry {
    const FRAME_MASK: u64 = 0x000F_FFFF_FFFF_F000;
    const FLAGS_MASK: u64 = !Self::FRAME_MASK;

    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    pub fn get(&self) -> Option<(Frame, PageTableFlags)> {
        let value = self.0.load(Ordering::Relaxed);
        if value == 0 {
            return None;
        }
        let frame = Frame::from_start_address(PhysAddr::new(value & Self::FRAME_MASK));
        let flags = PageTableFlags::from_bits(value & Self::FLAGS_MASK).unwrap();
        Some((frame, flags))
    }

    pub fn frame(&self) -> Option<Frame> {
        self.get().map(|x| x.0)
    }

    pub fn flags(&self) -> Option<PageTableFlags> {
        self.get().map(|x| x.1)
    }

    unsafe fn set_bits_mut(&mut self, bits: u64) -> Option<(Frame, PageTableFlags)> {
        let old = core::mem::replace(self.0.get_mut(), bits);

        if old == 0 {
            return None;
        }
        let addr = old & Self::FRAME_MASK;
        let attributes = PageTableFlags::from_bits(old & Self::FLAGS_MASK).unwrap();
        Some((Frame::from_start_address(PhysAddr::new(addr)), attributes))
    }

    unsafe fn set_bits(&self, bits: u64) -> Option<(Frame, PageTableFlags)> {
        let old = self.0.swap(bits, Ordering::Relaxed);

        if old == 0 {
            return None;
        }
        let addr = old & Self::FRAME_MASK;
        let attributes = PageTableFlags::from_bits(old & Self::FLAGS_MASK).unwrap();
        Some((Frame::from_start_address(PhysAddr::new(addr)), attributes))
    }

    unsafe fn try_set_bits(&self, bits: u64) -> Result<(), (Frame, PageTableFlags)> {
        self.0
            .compare_exchange(0, bits, Ordering::Relaxed, Ordering::Relaxed)
            .map(|_| ())
            .map_err(|old| {
                let addr = old & Self::FRAME_MASK;
                let attributes = PageTableFlags::from_bits(old & Self::FLAGS_MASK).unwrap();
                (Frame::from_start_address(PhysAddr::new(addr)), attributes)
            })
    }

    /// Atomically sets this entry to the frame and the attributes
    ///
    /// # Safety
    ///
    /// This could fundamentally change memory, leading to unsoundness.
    pub unsafe fn set(
        &self,
        frame: Frame,
        attributes: PageTableFlags,
    ) -> Option<(Frame, PageTableFlags)> {
        // SAFETY: Precondition
        unsafe { self.set_bits(attributes.bits() | frame.base().as_u64()) }
    }

    pub unsafe fn set_mut(
        &mut self,
        frame: Frame,
        attributes: PageTableFlags,
    ) -> Option<(Frame, PageTableFlags)> {
        // SAFETY: Precondition
        unsafe { self.set_bits(attributes.bits() | frame.base().as_u64()) }
    }

    /// Atomically sets this entry to the frame and the attributes if none present
    ///
    /// # Safety
    ///
    /// This could fundamentally change memory, leading to unsoundness.
    pub unsafe fn try_set(
        &self,
        frame: Frame,
        attributes: PageTableFlags,
    ) -> Result<(), (Frame, PageTableFlags)> {
        // SAFETY: Precondition
        unsafe { self.try_set_bits(attributes.bits() | frame.base().as_u64()) }
    }

    /// Atomically unsets this entry, leaving it empty
    ///
    /// # Safety
    ///
    /// This could fundamentally change memory, leading to unsoundness.
    pub unsafe fn reset(&self) -> Option<(Frame, PageTableFlags)> {
        // SAFETY: Precondition
        unsafe { self.set_bits(0) }
    }

    /// Atomically sets the flags on this entry (leaving the frame unchanged)
    ///
    /// # Safety
    ///
    /// This could fundamentally change memory, leading to unsoundness.
    pub unsafe fn set_flags(&self, flags: PageTableFlags) -> PageTableFlags {
        let old = self
            .0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some((value & Self::FRAME_MASK) | flags.bits())
            })
            .unwrap();

        PageTableFlags::from_bits(old & Self::FLAGS_MASK).unwrap()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PageTableOffset(u16);

#[derive(Debug, Copy, Clone)]
pub struct PageTableLevel(u8);

#[derive(Debug)]
pub struct InvalidLevel;

impl PageTableLevel {
    pub const fn new(level: u8) -> Self {
        match Self::try_new(level) {
            Ok(level) => level,
            Err(_) => panic!("Page table level must be within 1 and 4",),
        }
    }

    pub const fn try_new(level: u8) -> Result<Self, InvalidLevel> {
        if level < 1 || level > 4 {
            return Err(InvalidLevel);
        }
        Ok(Self(level))
    }

    pub const fn level(&self) -> u8 {
        self.0
    }

    pub const fn top() -> Self {
        Self(4)
    }

    pub const fn is_bottom(&self) -> bool {
        self.level() == 1
    }

    pub const fn lower(self) -> Option<Self> {
        match Self::try_new(self.level() - 1) {
            Ok(l) => Some(l),
            Err(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum PageTableOffsetError {
    OutOfBounds,
}

impl TryFrom<u16> for PageTableOffset {
    type Error = PageTableOffsetError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<usize> for PageTableOffset {
    type Error = PageTableOffsetError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(u16::try_from(value).map_err(|_| PageTableOffsetError::OutOfBounds)?)
    }
}

impl PageTableOffset {
    pub const fn new(offset: u16) -> Result<Self, PageTableOffsetError> {
        if offset < 512 {
            Ok(Self(offset))
        } else {
            Err(PageTableOffsetError::OutOfBounds)
        }
    }

    pub const fn is_lower_half(&self) -> bool {
        self.0 < 256
    }

    pub const fn new_truncate(addr: u16) -> Self {
        Self(addr % 512)
    }
}
