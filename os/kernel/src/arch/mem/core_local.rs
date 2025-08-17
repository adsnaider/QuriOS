use core::arch::asm;
use core::marker::PhantomData;
use core::ops::Deref;

pub struct CoreLocal<const BYTE_OFF: usize, T> {
    _data: PhantomData<*mut T>,
}

// SAFETY: The guarantee provided by CoreLocal is that, when properly constructed,
// each instance of CoreLocal<T> on its own CPU is unique. This guarantee implies
// Sync in the context of the kernel since it's non-preemptive.
unsafe impl<const BYTE_OFF: usize, T> Sync for CoreLocal<BYTE_OFF, T> {}

impl<const BYTE_OFF: usize, T> CoreLocal<BYTE_OFF, T> {
    /// Constructs a new CoreLocal shim to a piece of data in the CoreLocalData structure.
    ///
    /// # Safety
    ///
    /// * BYTE_OFF must point to a valid byte offset in the CoreLocalData field of type T.
    /// * Only 1 CoreLocal<T> instance per data field can be created.
    pub const unsafe fn new() -> Self {
        Self { _data: PhantomData }
    }

    pub fn get(&self) -> &T {
        // SAFETY: We never give mutable references and only a single instance of this struct can exist for this field.
        unsafe { &*Self::get_ptr() }
    }

    pub fn get_ptr() -> *const T {
        CoreLocalData::<()>::get_ptr().wrapping_byte_add(BYTE_OFF) as *const T
    }

    pub fn get_ptr_mut() -> *mut T {
        CoreLocalData::<()>::get_ptr_mut().wrapping_byte_add(BYTE_OFF) as *mut T
    }

    pub const fn offset(&self) -> usize {
        BYTE_OFF
    }
}

impl<const OFF: usize, T> Deref for CoreLocal<OFF, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct CoreLocalData<T> {
    self_ptr: *mut Self,
    pub data: T,
}

impl<T> CoreLocalData<T> {
    pub const fn new(self_addr: *mut Self, data: T) -> Self {
        Self {
            self_ptr: self_addr,
            data,
        }
    }
    pub fn get_ptr_mut() -> *mut Self {
        let ptr: *mut Self;
        // SAFETY: The first qword in the gs base will be the self-referencing pointer.
        unsafe {
            asm!("mov {}, qword ptr gs:[0]", lateout(reg) ptr);
        }
        debug_assert!(!ptr.is_null());
        ptr
    }

    pub fn get_ptr() -> *const Self {
        Self::get_ptr_mut() as *const _
    }
}
