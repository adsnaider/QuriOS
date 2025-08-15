use core::{
    any::TypeId,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

use sealed::sealed;

#[sealed]
pub trait InitState: 'static {}

pub struct Init;
pub struct Uninit;

#[sealed]
impl InitState for Init {}
#[sealed]
impl InitState for Uninit {}

#[repr(transparent)]
#[derive(Debug)]
pub struct DeferInit<T, State: InitState = Init>
where
    State: 'static,
{
    payload: MaybeUninit<T>,
    _state: PhantomData<State>,
}

impl<T> Default for DeferInit<T, Uninit> {
    fn default() -> Self {
        Self::uninit()
    }
}

impl<T: Default> Default for DeferInit<T, Init> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> DeferInit<T, Uninit> {
    pub const fn uninit() -> Self {
        Self {
            payload: MaybeUninit::uninit(),
            _state: PhantomData,
        }
    }

    pub const fn set(&mut self, payload: T) {
        self.payload.write(payload);
    }

    pub const unsafe fn assume_init_ref(&self) -> &DeferInit<T, Init> {
        unsafe { core::mem::transmute(self) }
    }

    pub const unsafe fn assume_init_mut(&mut self) -> &mut DeferInit<T, Init> {
        unsafe { core::mem::transmute(self) }
    }

    pub const unsafe fn assume_init(mut self) -> DeferInit<T, Init> {
        let payload = core::mem::replace(&mut self.payload, MaybeUninit::uninit());
        core::mem::forget(self);
        DeferInit {
            payload,
            _state: PhantomData,
        }
    }
}

impl<T> DeferInit<T, Init> {
    pub const fn new(payload: T) -> Self {
        Self {
            payload: MaybeUninit::new(payload),
            _state: PhantomData,
        }
    }
}

impl<T> AsRef<T> for DeferInit<T, Init> {
    fn as_ref(&self) -> &T {
        unsafe { self.payload.assume_init_ref() }
    }
}

impl<T> AsMut<T> for DeferInit<T, Init> {
    fn as_mut(&mut self) -> &mut T {
        unsafe { self.payload.assume_init_mut() }
    }
}

impl<T> Deref for DeferInit<T, Init> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<T> DerefMut for DeferInit<T, Init> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

impl<State: InitState, T> Drop for DeferInit<T, State> {
    fn drop(&mut self) {
        if TypeId::of::<State>() == TypeId::of::<Init>() {
            unsafe { self.payload.assume_init_drop() };
        }
    }
}
