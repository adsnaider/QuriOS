use core::alloc::Layout;
use core::mem::MaybeUninit;
use core::ops::{BitOr, BitOrAssign};
use core::ptr::NonNull;

use allocator_api2::alloc::{AllocError, Allocator};
use derive_more::{Deref, DerefMut, Display, Error};
use heapless::Vec;
use linked_list_allocator::Heap;
use qapi::mem::{Frame, Page, PageFlags};

use super::cspace::{CAllocError, CSpace, CapNode, CapabilityMan};
use super::pmspace::bitmap_allocator::BitmapAllocator;
use super::pmspace::{FrameAllocError, PMSpace};
use super::vmspace::{Addrspace, VMSpace};

pub struct Allocman<C = CapabilityMan, P = BitmapAllocator, V = Addrspace> {
    cspace: C,
    pmspace: P,
    mspace: V,

    resources: Resources,
}

// SAFETY: The *mut u8 heap_start has no ownership semantics. Is effectively a const
unsafe impl Send for Resources {}

#[derive(Debug, Deref, DerefMut)]
pub struct Mutex<T: ?Sized>(spin::Mutex<T>);

pub(super) struct Resources {
    cspace: heapless::Vec<CapNode, 16>,
    pmspace: heapless::Vec<Frame, 16>,
    fixed_heap: Heap,
    main_heap: Heap,
    heap_start: *mut u8,
}

impl Resources {
    fn steal_cap(&mut self) -> Result<CapNode, CAllocError> {
        self.cspace.pop().ok_or(CAllocError::CapTreeFull)
    }

    fn steal_frame(&mut self) -> Result<Frame, FrameAllocError> {
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

    pub fn extend_heap<V: VMSpace, P: PMSpace>(
        &mut self,
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
            let top = if self.main_heap.bottom().is_null() {
                self.heap_start
            } else {
                self.main_heap.top()
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
                self,
            ) else {
                pmspace.dealloc(frame);
                return progress;
            };
            if self.main_heap.bottom().is_null() {
                // SAFETY: Properly allocated above
                self.main_heap = unsafe { Heap::new(self.heap_start, Page::SIZE) };
            } else {
                // SAFETY: Properly allocated above
                unsafe { self.main_heap.extend(Page::SIZE) };
            }
            progress |= Progress::Progress;
            log::debug!("Extended heap by {i} of {pages} pages");
        }
        progress
    }
}

unsafe impl Allocator for &'static Mutex<Resources> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.lock().alloc_mem(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: Precondition
        unsafe { self.0.lock().dealloc_mem(ptr, layout) }
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
    pub fn new(
        cspace: C,
        utspace: PM,
        mspace: VM,
        fixed_pool: &'static mut [MaybeUninit<u8>],
        heap_start: *mut u8,
    ) -> Self {
        let heap = Heap::from_slice(fixed_pool);

        let mut this = Self {
            cspace,
            pmspace: utspace,
            mspace,
            resources: Resources {
                cspace: Vec::new(),
                pmspace: Vec::new(),
                fixed_heap: heap,
                heap_start,
                main_heap: Heap::empty(),
            },
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
        const MIN_HEAP_FREE: usize = 10 * Page::SIZE;
        if self.resources.main_heap.free() < MIN_HEAP_FREE {
            let pages = (MIN_HEAP_FREE - self.resources.main_heap.free()) / Page::SIZE;
            let progress = self
                .resources
                .extend_heap(&mut self.mspace, &mut self.pmspace, pages);
            progress
        } else {
            Progress::NoProgress
        }
    }

    fn refill_cspace(&mut self) -> Progress {
        let mut progress = Progress::NoProgress;
        while !self.resources.cspace.is_full() {
            let cnode = match self.cspace.alloc_cap(&mut self.resources) {
                Ok(cnode) => cnode,
                Err(e) => {
                    log::warn!("Couldn't allocate cnode: {e}");
                    return progress;
                }
            };
            self.resources
                .cspace
                .push(cnode)
                .unwrap_or_else(|_| panic!("cspace isn't full so this must always succeed"));
            progress = progress | Progress::Progress;
        }
        progress
    }

    fn refill_utspace(&mut self) -> Progress {
        let mut progress = Progress::NoProgress;
        while !self.resources.pmspace.is_full() {
            let frame = match self.pmspace.alloc_frame() {
                Ok(cnode) => cnode,
                Err(e) => {
                    log::warn!("Couldn't allocate cnode: {e}");
                    return progress;
                }
            };
            self.resources
                .pmspace
                .push(frame)
                .unwrap_or_else(|_| panic!("cspace isn't full so this must always succeed"));
            progress = progress | Progress::Progress;
        }
        progress
    }

    pub fn mem_alloc(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        todo!();
    }

    pub unsafe fn mem_dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
        todo!();
    }
}
