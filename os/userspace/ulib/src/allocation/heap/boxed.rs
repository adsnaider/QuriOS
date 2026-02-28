use core::alloc::Layout;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::ptr::{self, NonNull, drop_in_place};

use crate::allocation::heap::undrop::PhantomUndrop;

unsafe impl<T: Send> Send for Carton<T> {}
unsafe impl<T: Sync> Sync for Carton<T> {}

#[must_use]
#[derive(Debug)]
pub struct Carton<T: ?Sized> {
    ptr: NonNull<T>,
    no_drop: PhantomUndrop,
}

impl<T> Carton<T> {
    pub fn new(data: T, allocator: &mut impl Alloc) -> Result<Self, AllocError> {
        let layout = Layout::new::<T>();
        let memory = allocator.alloc(layout)?;
        let data = ManuallyDrop::new(data);
        debug_assert!(memory.len() >= size_of::<T>());
        let memory = memory.cast();
        debug_assert!(memory.is_aligned());
        unsafe { ptr::copy_nonoverlapping(&*data, memory.as_ptr() as *mut T, 1) };
        Ok(Self { ptr: memory })
    }
}
impl<T: ?Sized> Carton<T> {
    // SAFETY: The allocator must be the same one used for construction.
    pub unsafe fn drop(self, allocator: &mut impl Alloc) {
        let Self { ptr } = self;
        let layout = unsafe { Layout::for_value_raw(ptr.as_ptr()) };
        unsafe { drop_in_place(ptr.as_ptr()) };
        unsafe { allocator.dealloc(ptr.cast(), layout) };
    }
}

impl<T: ?Sized> Deref for Carton<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T: ?Sized> DerefMut for Carton<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut() }
    }
}

impl<T: ?Sized> Drop for Carton<T> {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        panic!("Dropping a `Carton` implies a memory leak has ocurred. Use `.drop()` instead");
    }
}
