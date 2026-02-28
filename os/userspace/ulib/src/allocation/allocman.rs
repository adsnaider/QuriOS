use core::alloc::{AllocError, Allocator, Layout};
use core::cell::RefCell;
use core::mem::MaybeUninit;
use core::ops::{BitOr, BitOrAssign, Deref, DerefMut};
use core::ptr::NonNull;

use derive_more::{Deref, DerefMut, Display, Error};
use heapless::Vec;
use linked_list_allocator::Heap;
use qapi::caps::CapId;
use qapi::mem::{Frame, Page, PageFlags};
use spin::Mutex;

use super::cspace::{CAllocError, CSpace, CapabilityMan};
use super::pmspace::bitmap_allocator::BitmapAllocator;
use super::pmspace::{FrameAllocError, PMSpace};
use super::vmspace::{Addrspace, VMSpace};
use crate::allocation::cspace::CapSlot;

pub struct Allocman<C = CapabilityMan, P = BitmapAllocator, V = Addrspace> {
    cspace: C,
    pmspace: P,
    mspace: V,

    resources: &'static SharedResources,
}

pub struct SharedResources(Mutex<Resources>);

impl SharedResources {
    pub fn borrow_mut(&self) -> impl DerefMut<Target = Resources> + '_ {
        self.0.try_lock().unwrap()
    }

    pub fn borrow(&self) -> impl Deref<Target = Resources> + '_ {
        self.0.try_lock().unwrap()
    }
}

unsafe impl Allocator for SharedResources {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.borrow_mut().alloc_mem(layout)
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: Layout) {
        unsafe { self.borrow_mut().dealloc_mem(ptr, layout) }
    }
}

// SAFETY: The *mut u8 heap_start has no ownership semantics. Is effectively a const
unsafe impl Send for Resources {}

pub struct Resources {
    cspace: heapless::Vec<CapSlot, 16>,
    pmspace: heapless::Vec<Frame, 16>,
    fixed_heap: Heap,
    main_heap: Heap,
    heap_start: *mut u8,
}

impl Resources {
    pub fn new(fixed_pool: &'static mut [MaybeUninit<u8>], heap_start: *mut u8) -> Self {
        let heap = Heap::from_slice(fixed_pool);
        Self {
            cspace: Vec::new(),
            pmspace: Vec::new(),
            fixed_heap: heap,
            heap_start,
            main_heap: Heap::empty(),
        }
    }
    pub fn steal_cap(&mut self) -> Result<CapSlot, CAllocError> {
        self.cspace.pop().ok_or(CAllocError::CapTreeFull)
    }

    pub fn steal_frame(&mut self) -> Result<Frame, FrameAllocError> {
        self.pmspace.pop().ok_or(FrameAllocError::OutOfFrames)
    }

    pub fn alloc_mem(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.main_heap
            .allocate_first_fit(layout)
            .or_else(|_| self.fixed_heap.allocate_first_fit(layout))
            .map_err(|_| AllocError)
            .map(|ptr| NonNull::slice_from_raw_parts(ptr, layout.size()))
    }

    pub unsafe fn dealloc_mem(&mut self, ptr: NonNull<u8>, layout: Layout) {
        let main_range = self.main_heap.bottom()..self.main_heap.top();
        let fixed_range = self.fixed_heap.bottom()..self.fixed_heap.top();
        if main_range.contains(&ptr.as_ptr()) {
            // SAFETY: Precondition and checked the range
            unsafe { self.main_heap.deallocate(ptr, layout) };
        } else {
            debug_assert!(fixed_range.contains(&ptr.as_ptr()));
            unsafe { self.fixed_heap.deallocate(ptr, layout) };
        }
    }
}

impl SharedResources {
    pub const fn new(resources: Resources) -> Self {
        Self(Mutex::new(resources))
    }

    pub fn extend_heap<V: VMSpace, P: PMSpace>(
        &self,
        vmspace: &mut V,
        pmspace: &mut P,
        pages: usize,
    ) -> Progress {
        let mut progress = Progress::NoProgress;
        for i in 0..pages {
            let Ok(frame) = pmspace
                .alloc_frame()
                .inspect_err(|e| log::warn!("Couldn't allocate an untyped frame: {e}"))
            else {
                return progress;
            };

            // Gotta create the heap instead
            let top = if self.borrow().main_heap.bottom().is_null() {
                self.borrow().heap_start
            } else {
                self.borrow().main_heap.top()
            };
            let page = Page::try_new(top as usize).unwrap();
            let Ok(()) = vmspace.map_to(
                page,
                frame,
                PageFlags::READABLE | PageFlags::WRITABLE | PageFlags::PRESENT,
                PageFlags::READABLE
                    | PageFlags::WRITABLE
                    | PageFlags::EXECUTABLE
                    | PageFlags::PRESENT,
            ) else {
                unsafe { pmspace.dealloc(frame) };
                return progress;
            };
            if self.borrow().main_heap.bottom().is_null() {
                // SAFETY: Properly allocated above
                self.borrow_mut().main_heap =
                    unsafe { Heap::new(self.borrow().heap_start, Page::SIZE) };
            } else {
                // SAFETY: Properly allocated above
                unsafe { self.borrow_mut().main_heap.extend(Page::SIZE) };
            }
            progress |= Progress::Progress;
            log::debug!("Extended heap by {i} of {pages} pages");
        }
        progress
    }
}

#[derive(Debug, Error, Display)]
pub enum ResourceExhaustion {
    #[display("Error extending the main heap")]
    OutOfMemory,
    #[display("Error allocating C-Nodes")]
    CNodeAllocError(CAllocError),
    #[display("Error allocating UT-Nodes (page frames)")]
    FrameAllocError(FrameAllocError),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Progress {
    NoProgress,
    Progress,
}

impl Progress {
    pub fn is_stalled(&self) -> bool {
        *self == Self::NoProgress
    }
}

impl BitOr for Progress {
    type Output = Progress;

    fn bitor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Progress::NoProgress, Progress::NoProgress) => Progress::NoProgress,
            (Progress::NoProgress, Progress::Progress) => Progress::Progress,
            (Progress::Progress, Progress::NoProgress) => Progress::Progress,
            (Progress::Progress, Progress::Progress) => Progress::Progress,
        }
    }
}

impl BitOrAssign for Progress {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl<C: CSpace, PM: PMSpace, VM: VMSpace> Allocman<C, PM, VM> {
    pub fn new(cspace: C, utspace: PM, mspace: VM, resources: &'static SharedResources) -> Self {
        let mut this = Self {
            cspace,
            pmspace: utspace,
            mspace,
            resources,
        };

        this.refill_resources();
        this
    }

    pub fn refill_resources(&mut self) {
        const ITERATION_LIMIT: usize = 4;
        let mut i = 0;
        loop {
            log::debug!("Refilling resources {i}/{ITERATION_LIMIT}");
            i += 1;
            let progress = self.refill_heap() | self.refill_cspace() | self.refill_utspace();
            if progress.is_stalled() || i >= ITERATION_LIMIT {
                break;
            }
        }
    }

    fn refill_heap(&mut self) -> Progress {
        log::debug!("Refilling the heap");
        const MIN_HEAP_FREE: usize = 10 * Page::SIZE;
        if self.resources.borrow().main_heap.free() < MIN_HEAP_FREE {
            log::debug!(
                "Available free space: {}",
                self.resources.borrow_mut().main_heap.free()
            );
            let pages = (MIN_HEAP_FREE - self.resources.borrow().main_heap.free()) / Page::SIZE;
            self.resources
                .extend_heap(&mut self.mspace, &mut self.pmspace, pages)
        } else {
            Progress::NoProgress
        }
    }

    fn refill_cspace(&mut self) -> Progress {
        log::debug!("Refilling capability space");
        let mut progress = Progress::NoProgress;
        let mut i = 0;
        let iters = {
            let resources = self.resources.borrow();
            resources.cspace.capacity() - resources.cspace.len()
        };
        while !self.resources.borrow().cspace.is_full() {
            i += 1;
            log::debug!("Refilling capability space: {i}/{}", iters);
            let cnode = match self.cspace.alloc_cap() {
                Ok(cnode) => cnode,
                Err(e) => {
                    log::warn!("Couldn't allocate cnode: {e}");
                    return progress;
                }
            };
            self.resources
                .borrow_mut()
                .cspace
                .push(cnode)
                .unwrap_or_else(|_| panic!("cspace isn't full so this must always succeed"));
            progress |= Progress::Progress;
        }
        progress
    }

    fn refill_utspace(&mut self) -> Progress {
        let mut progress = Progress::NoProgress;
        log::debug!("Refilling physical memory");
        let iters = {
            let resources = self.resources.borrow();
            resources.pmspace.capacity() - resources.pmspace.len()
        };
        let mut i = 0;
        while !self.resources.borrow().pmspace.is_full() {
            i += 1;
            log::debug!("Refilling physical memory: {i}/{}", iters,);
            let frame = match self.pmspace.alloc_frame() {
                Ok(frame) => frame,
                Err(e) => {
                    log::warn!("Couldn't allocate frame: {e}");
                    return progress;
                }
            };
            log::debug!("B");
            self.resources
                .borrow_mut()
                .pmspace
                .push(frame)
                .unwrap_or_else(|_| panic!("cspace isn't full so this must always succeed"));
            log::debug!("C");
            progress |= Progress::Progress;
            log::debug!("D");
        }
        progress
    }

    pub fn mem_alloc(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.refill_resources();
        self.resources.borrow_mut().alloc_mem(layout)
    }

    pub unsafe fn mem_free(&mut self, ptr: NonNull<u8>, layout: Layout) {
        self.refill_resources();
        // SAFETY: Precondition
        unsafe { self.resources.borrow_mut().dealloc_mem(ptr, layout) }
    }

    pub fn cap_alloc(&mut self) -> Result<CapSlot, CAllocError> {
        self.refill_resources();
        self.cspace.alloc_cap()
    }

    pub unsafe fn cap_free(&mut self, node: CapId) {
        self.refill_resources();
        // SAFETY: Precondition
        unsafe { self.cspace.cap_free(node) }
    }

    pub fn frame_alloc(&mut self) -> Result<Frame, FrameAllocError> {
        self.refill_resources();
        self.pmspace.alloc_frame()
    }

    pub unsafe fn frame_dealloc(&mut self, frame: Frame) {
        self.refill_resources();
        // SAFETY: Precondition
        unsafe { self.pmspace.dealloc(frame) }
    }
}
