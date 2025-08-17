use core::marker::PhantomData;

use zerocopy::{Immutable, IntoBytes, KnownLayout};

#[repr(transparent)]
#[derive(IntoBytes, KnownLayout, Immutable)]
pub struct UserPtr<T> {
    addr: usize,
    _phantom: PhantomData<*const T>,
}

impl<T> UserPtr<T> {
    pub fn new(ptr: *const T) -> Self {
        Self {
            addr: ptr.addr(),
            _phantom: PhantomData,
        }
    }

    pub fn from_addr(addr: usize) -> Self {
        Self {
            addr,
            _phantom: PhantomData,
        }
    }

    pub fn addr(&self) -> usize {
        self.addr
    }

    pub fn cast<U>(self) -> UserPtr<U> {
        UserPtr {
            addr: self.addr,
            _phantom: PhantomData,
        }
    }

    pub fn as_ptr(&self) -> *const T {
        self.addr as *const T
    }
}

#[repr(C)]
#[derive(KnownLayout, Immutable)]
pub struct CSlice<'a, T> {
    ptr: UserPtr<T>,
    length: usize,
    _cont: PhantomData<&'a [T]>,
}

// SAFETY: IntoBytes is reasonable in this case since the data is just a pointer
// and length with repr(C). The resulting type has no padding and there's no
// interior mutability
unsafe impl<'a, T> IntoBytes for CSlice<'a, T> {
    fn only_derive_is_allowed_to_implement_this_trait()
    where
        Self: Sized,
    {
    }
}

impl<T> Copy for CSlice<'static, T> {}
impl<T> Clone for CSlice<'static, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for UserPtr<T> {}
impl<T> Clone for UserPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> core::fmt::Debug for UserPtr<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OrphanPtr")
            .field("addr", &self.addr)
            .finish()
    }
}
impl<T> core::fmt::Debug for CSlice<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CSlice")
            .field("ptr", &self.ptr)
            .field("length", &self.length)
            .finish()
    }
}

impl<'a, T> CSlice<'a, T> {
    /// Constructs a CSlice from the provided pointer and length
    ///
    /// # Safety
    ///
    /// The pointer and length must denote a valid slice and the lifetime
    /// created must be strictly smaller or equal to the lifetime of the underlying
    /// data
    pub unsafe fn from_raw_parts(ptr: *const T, length: usize) -> Self {
        Self {
            ptr: UserPtr::new(ptr),
            length,
            _cont: PhantomData,
        }
    }

    pub fn from_slice(s: &'a [T]) -> Self {
        // SAFETY: Slice is valid
        unsafe { Self::from_raw_parts(s.as_ptr(), s.len()) }
    }

    pub fn ptr(&self) -> *const T {
        self.ptr.addr as *const T
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn as_slice(&self) -> &'a [T] {
        // SAFETY: CSlice must be valid from precondition at construction
        unsafe { core::slice::from_raw_parts(self.ptr(), self.len()) }
    }
}

impl<T> From<UserPtr<T>> for usize {
    fn from(value: UserPtr<T>) -> Self {
        value.addr()
    }
}

impl<T> From<usize> for UserPtr<T> {
    fn from(value: usize) -> Self {
        Self::from_addr(value)
    }
}
