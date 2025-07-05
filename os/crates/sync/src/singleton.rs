use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

/// The inverse of a once cell, starts with a value that gets taken.
pub struct Singleton<T> {
    value: UnsafeCell<MaybeUninit<T>>,
    taken: AtomicBool,
}

// SAFETY: No references are ever exposed, but a reference to Singleton would
// enable sending the data inside it.
unsafe impl<T: Send> Sync for Singleton<T> {}
// SAFETY: Sending a singleton causes the data inside to potentially move
unsafe impl<T: Send> Send for Singleton<T> {}

impl<T> Singleton<T> {
    pub const fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(MaybeUninit::new(value)),
            taken: AtomicBool::new(false),
        }
    }

    pub fn take(&self) -> Option<T> {
        if !self.taken.swap(true, Ordering::Relaxed) {
            // SAFETY: has not been consumed and no reace condition due to atomic swap.
            Some(unsafe { self.take_value() })
        } else {
            None
        }
    }

    pub fn into_inner(self) -> Option<T> {
        self.taken
            .into_inner()
            // SAFETY: We have ownership of the entire singleton and value hasn't been consumed
            .then(|| unsafe { self.value.into_inner().assume_init() })
    }

    pub fn take_mut(&mut self) -> Option<T> {
        let taken = self.taken.get_mut();

        if *taken {
            None
        } else {
            *taken = true;
            // SAFETY: We have mutable ownership of the entire singleton and value hasn't been consumed
            Some(unsafe { self.take_value() })
        }
    }

    /// # Safety
    ///
    /// Value must not have been previously taken and there can't be race conditions.
    unsafe fn take_value(&self) -> T {
        core::ptr::replace(self.value.get(), MaybeUninit::uninit()).assume_init()
    }
}
