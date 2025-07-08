use core::marker::PhantomData;

#[repr(transparent)]
pub struct OrphanPtr<T> {
    addr: usize,
    _phantom: PhantomData<*const T>,
}
impl<T> OrphanPtr<T> {
    fn new(ptr: *const T) -> Self {
        Self {
            addr: ptr.addr(),
            _phantom: PhantomData,
        }
    }
}

#[repr(C)]
pub struct CSlice<'a, T> {
    ptr: OrphanPtr<T>,
    length: usize,
    _cont: PhantomData<&'a [T]>,
}

impl<T> Copy for CSlice<'static, T> {}
impl<T> Clone for CSlice<'static, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for OrphanPtr<T> {}
impl<T> Clone for OrphanPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> core::fmt::Debug for OrphanPtr<T> {
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
    pub unsafe fn from_raw_parts(ptr: *const T, length: usize) -> Self {
        Self {
            ptr: OrphanPtr::new(ptr),
            length,
            _cont: PhantomData,
        }
    }

    pub fn from_slice(s: &'a [T]) -> Self {
        unsafe { Self::from_raw_parts(s.as_ptr(), s.len()) }
    }

    pub fn ptr(&self) -> *const T {
        self.ptr.addr as *const T
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn into_slice(self) -> &'a [T] {
        unsafe { core::slice::from_raw_parts(self.ptr(), self.len()) }
    }
}
