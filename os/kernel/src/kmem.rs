//! Kernel-owned memory utilities

use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::ptr::NonNull;

use crate::{
    pmo::{PhysAddrExt as _, VirtAddrExt as _},
    retyping::{AsUnusedKernelError, FrameExt, KernelFrame},
};
use arch::mem::{Frame, Page, VirtAddr};

/// A "kernel" pointer to any page-aligned resource.
///
/// A kernel pointer is a pointer type that may only point to a kernel object
/// that takes up an entire page. This is because kernel pointers use the
/// memroy retyping capabilities which use reference counts on an entire
/// page.
#[repr(transparent)]
pub struct KPtr<T> {
    inner: NonNull<T>,
}

impl<T> core::fmt::Debug for KPtr<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KPtr").field("inner", &self.inner).finish()
    }
}

impl<T> PartialEq for KPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}
impl<T> Eq for KPtr<T> {}

// SAFETY: Similar restrictions to Arc<T>
unsafe impl<T: Send + Sync> Send for KPtr<T> {}
// SAFETY: Similar restrictions to Arc<T>
unsafe impl<T: Send + Sync> Sync for KPtr<T> {}

impl<T> KPtr<T> {
    const VALID_SIZE_AND_ALIGN: () = {
        assert!(core::mem::size_of::<T>() <= Page::SIZE);
        assert!(Page::SIZE % core::mem::align_of::<T>() == 0);
    };
    const TRIVIALLY_DROPPABLE: () = {
        assert!(!core::mem::needs_drop::<T>());
    };

    #[inline(always)]
    pub fn new(frame: Frame, value: T) -> Result<Self, AsUnusedKernelError> {
        let () = Self::VALID_SIZE_AND_ALIGN;
        let () = Self::TRIVIALLY_DROPPABLE;
        // SAFETY: Frame is typed as kernel and atomically incremented
        unsafe { Ok(Self::new_unchecked(frame.try_as_unused_kernel()?, value)) }
    }

    /// Constructs a KPtr from some pretyped kernel frame
    ///
    /// # Safety
    ///
    /// Unused kernel frames must be used. This can generally be guaranteed if
    /// the frame is retyped into a kernel frame as opposed to using a pre-allocated
    /// kernel frame
    #[inline(always)]
    pub unsafe fn new_unchecked(frame: KernelFrame, value: T) -> Self {
        let frame = frame.into_raw();
        let pointer = frame.addr().to_virtual().as_mut_ptr();
        let ptr: NonNull<T> = NonNull::new(pointer).unwrap();
        debug_assert!(ptr.as_ptr() as usize % Page::SIZE == 0);
        // SAFETY: Allocation is well-aligned and sufficiently large for T
        unsafe {
            let () = Self::VALID_SIZE_AND_ALIGN;
            ptr.as_ptr().write(value);
        }
        Self { inner: ptr }
    }

    /// # Safety
    ///
    /// The frame must only be used by `KPtr<T>`
    pub unsafe fn from_frame_unchecked(frame: KernelFrame) -> Self {
        let frame = frame.into_raw();
        let pointer = frame.addr().to_virtual().as_mut_ptr();
        let ptr = NonNull::new(pointer).unwrap();
        assert!(ptr.as_ptr() as usize % Page::SIZE == 0);
        Self { inner: ptr }
    }

    /// # Safety
    ///
    /// Pointer must be a valid kernel struct originally constructed as a KPtr
    pub unsafe fn from_ptr_unchecked(value: NonNull<T>) -> Self {
        let this = Self { inner: value };
        this.frame().as_kernel_unchecked().into_raw();
        this
    }

    pub fn frame(&self) -> Frame {
        // SAFETY: Pointer was created a physical address
        Frame::from_start_address(unsafe { VirtAddr::from_ptr(self.inner.as_ptr()).to_physical() })
    }

    pub fn into_raw(self) -> Frame {
        let this = ManuallyDrop::new(self);
        this.frame()
    }
}

impl<T> AsRef<T> for KPtr<T> {
    fn as_ref(&self) -> &T {
        // SAFETY: KPtr provides shared semantics akin to Arc.
        unsafe { self.inner.as_ref() }
    }
}

impl<T> Deref for KPtr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: KPtr provides shared semantics akin to Arc.
        unsafe { self.inner.as_ref() }
    }
}

impl<T> Clone for KPtr<T> {
    fn clone(&self) -> Self {
        // SAFETY: Ownership of a KPtr guarantees the frame is properly typed as a kernel
        let frame = unsafe { self.frame().as_kernel_unchecked() };
        // SAFETY: The frame is coming from a KPtr of the same type.
        unsafe { Self::from_frame_unchecked(frame) }
    }
}

impl<T> Drop for KPtr<T> {
    fn drop(&mut self) {
        // SAFETY: We constructed the frame with `into_raw`.
        unsafe { KernelFrame::from_raw(self.frame()).drop() };
        let () = Self::TRIVIALLY_DROPPABLE;
    }
}
