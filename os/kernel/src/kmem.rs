//! Kernel-owned memory utilities

use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::ptr::NonNull;
use core::sync::atomic::{fence, Ordering};

use crate::retyping::{AsUnusedKernelError, FrameExt, KernelFrame};
use crate::util::{PhysAddrExt as _, VirtAddrExt};
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

unsafe impl<T: Send> Send for KPtr<T> {}
unsafe impl<T: Sync> Sync for KPtr<T> {}

impl<T> KPtr<T> {
    const VALID_SIZE_AND_ALIGN: () = {
        assert!(core::mem::size_of::<T>() <= Page::SIZE);
        assert!(Page::SIZE % core::mem::align_of::<T>() == 0);
    };

    #[inline(always)]
    pub fn new(frame: Frame, value: T) -> Result<Self, AsUnusedKernelError> {
        let () = Self::VALID_SIZE_AND_ALIGN;
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
        assert!(ptr.as_ptr() as usize % Page::SIZE == 0);
        unsafe {
            ptr.as_ptr().write(value);
        }
        Self { inner: ptr }
    }

    /// # Safety
    ///
    /// The frame must only be used by KPtr<T>
    pub unsafe fn from_frame_unchecked(frame: KernelFrame) -> Self {
        let frame = frame.into_raw();
        let pointer = frame.addr().to_virtual().as_mut_ptr();
        let ptr = NonNull::new(pointer).unwrap();
        assert!(ptr.as_ptr() as usize % Page::SIZE == 0);
        Self { inner: ptr }
    }

    pub fn frame(&self) -> Frame {
        // SAFETY: Pointer was created a physical address
        Frame::from_start_address(unsafe { VirtAddr::from_ptr(self.inner.as_ptr()).to_physical() })
    }

    pub fn try_into_inner(self) -> Option<T> {
        // SAFETY: The frame must be typed as kernel since we have a reference
        // to it.
        let count = unsafe { KernelFrame::from_raw(self.frame()).drop() };
        if count == 1 {
            // last one turns off the lights
            Some(unsafe { self.inner.as_ptr().read() })
        } else {
            None
        }
    }

    pub fn into_raw(self) -> Frame {
        let this = ManuallyDrop::new(self);
        this.frame()
    }
}

impl<T> AsRef<T> for KPtr<T> {
    fn as_ref(&self) -> &T {
        unsafe { self.inner.as_ref() }
    }
}

impl<T> Deref for KPtr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.inner.as_ref() }
    }
}

impl<T> Clone for KPtr<T> {
    fn clone(&self) -> Self {
        let frame = unsafe { self.frame().as_kernel_unchecked() };
        // SAFETY: The frame is coming from a KPtr of the same type.
        unsafe { Self::from_frame_unchecked(frame) }
    }
}

impl<T> Drop for KPtr<T> {
    fn drop(&mut self) {
        let count = unsafe { KernelFrame::from_raw(self.frame()).drop() };
        // last one turns off the lights
        if count == 1 {
            fence(Ordering::Acquire);
            log::trace!("Last ones! Dropping T");
            unsafe {
                self.inner.as_ptr().drop_in_place();
            }
        }
    }
}
