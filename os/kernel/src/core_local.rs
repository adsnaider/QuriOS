use core::ops::Deref;

// TODO: This is likely going to use GSBase to get the correct value
#[repr(transparent)]
pub struct CoreLocal<T> {
    data: T,
}

// SAFETY: Since this is strictly core-local there won't be any race conditions
// This is due to also the fact that the kenrel doesn't have internal threads.
unsafe impl<T> Sync for CoreLocal<T> {}

impl<T> CoreLocal<T> {
    pub const fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T> Deref for CoreLocal<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
