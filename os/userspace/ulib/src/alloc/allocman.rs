#![allow(unused)]
use core::alloc::{GlobalAlloc, Layout};
use core::marker::PhantomPinned;
use core::ptr::NonNull;

use allocator_api2::alloc::{AllocError, Allocator};
use derive_more::{Deref, DerefMut, Display, Error};
use linked_list_allocator::{Heap, LockedHeap};

use super::caps::{CAllocError, CapNode, CapabilityMan};
use super::phys::FrameAllocator;
use super::virt::Addrspace;

#[derive(Deref, DerefMut)]
pub struct ALockedMan<F: FrameAllocator>(spin::Mutex<Option<Allocman<F>>>);

impl<F: FrameAllocator> ALockedMan<F> {
    pub const fn new(allocman: Allocman<F>) -> Self {
        Self(spin::Mutex::new(Some(allocman)))
    }

    pub const fn uninit() -> Self {
        Self(spin::Mutex::new(None))
    }

    pub fn set(&self, allocman: Allocman<F>) {
        assert!(
            self.lock().replace(allocman).is_none(),
            "Double initialization of Allocman"
        );
    }
}

pub struct ReservedHeap(LockedHeap);

impl ReservedHeap {
    pub const fn empty() -> Self {
        Self(LockedHeap::empty())
    }

    pub fn hydrate_reserves<F: FrameAllocator>(&self, f: &F) -> Result<(), HeapError> {
        todo!();
    }
}

// SAFETY: Implementation of `allocate`/`deallocate` is correct as per `LockedHeap`
unsafe impl Allocator for ReservedHeap {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        match self.0.lock().allocate_first_fit(layout) {
            Ok(ptr) => Ok(NonNull::slice_from_raw_parts(ptr, layout.size())),
            Err(()) => Err(AllocError),
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: Precondition
        unsafe { self.0.lock().deallocate(ptr, layout) }
    }
}

pub struct Allocman<F: FrameAllocator> {
    falloc: F,
    addrspace: Addrspace<ReservedHeap>,
    caps: CapabilityMan<ReservedHeap>,
    main_heap: Heap,
    _pinned: PhantomPinned,
}

impl<F: FrameAllocator> Allocman<F> {
    pub fn new(
        falloc: F,
        addrspace: Addrspace<ReservedHeap>,
        caps: CapabilityMan<ReservedHeap>,
    ) -> Result<Self, HeapError> {
        let mut this = Self {
            falloc,
            addrspace,
            caps,
            main_heap: Heap::empty(),
            _pinned: PhantomPinned,
        };
        this.hydrate_reserves()?;
        Ok(this)
    }

    pub fn malloc(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        loop {
            match self.main_heap.allocate_first_fit(layout) {
                Ok(ptr) => return Ok(NonNull::slice_from_raw_parts(ptr, layout.size())),
                Err(()) => self.try_extend_heap()?,
            }
        }
    }

    fn try_extend_heap(&mut self) -> Result<(), HeapError> {
        let frame = self.falloc.alloc().ok_or(HeapError::OutOfMemory)?;
        todo!();
        self.hydrate_reserves()
    }

    fn hydrate_reserves(&mut self) -> Result<(), HeapError> {
        self.addrspace.allocator().hydrate_reserves(&self.falloc)?;
        self.caps.allocator().hydrate_reserves(&self.falloc)?;
        Ok(())
    }

    /// # Safety
    ///
    /// `ptr` must be a pointer returned by a call to the [`malloc`] function with
    /// identical layout. Undefined behavior may occur for invalid arguments.
    pub unsafe fn free(&mut self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: Precondition
        unsafe { self.main_heap.deallocate(ptr, layout) }
    }

    pub fn cap_alloc(&mut self) -> Result<CapNode, CAllocError> {
        self.caps.alloc_cap()
    }

    /// # Safety
    ///
    /// Capabilities may cause underlying changes to the memory space. Care must
    /// be taken to guarantee this is safe
    pub unsafe fn cap_free(&mut self, cap: CapNode) {
        self.caps.cap_free(cap)
    }
}

#[derive(Debug, Clone, Error, Display)]
pub enum HeapError {
    #[display("Out of available memory in the component")]
    OutOfMemory,
}

impl From<HeapError> for AllocError {
    fn from(_value: HeapError) -> Self {
        Self
    }
}

// SAFETY: Allocman is faithful to the GlobalAlloc safety requirements
unsafe impl<F: FrameAllocator> GlobalAlloc for ALockedMan<F> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.lock()
            .as_mut()
            .unwrap()
            .malloc(layout)
            .map(|p| p.as_ptr() as *mut u8)
            .unwrap_or(core::ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(ptr) = NonNull::new(ptr) else {
            return;
        };
        // SAFETY: Precondition
        unsafe { self.lock().as_mut().unwrap().free(ptr, layout) };
    }
}

// SAFETY: Allocman is faithful to the Allocator safety requirements
unsafe impl<F: FrameAllocator> Allocator for ALockedMan<F> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.lock().as_mut().unwrap().malloc(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: Precondition
        unsafe { self.lock().as_mut().unwrap().free(ptr, layout) }
    }
}
