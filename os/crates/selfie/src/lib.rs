#![no_std]

pub mod uninit;

use derive_more::Deref;

#[repr(transparent)]
#[derive(Debug, Deref)]
pub struct SelfLife<T: 'static>(&'static T);

impl<T: 'static> SelfLife<T> {
    pub fn new(data: &T) -> Self {
        let data = unsafe { core::mem::transmute::<&T, &'static T>(data) };
        Self(data)
    }
}

#[cfg(test)]
mod tests {
    use core::marker::PhantomPinned;
    use core::pin::Pin;

    use super::*;
    use crate::uninit::{DeferInit, Init, InitState, Uninit};

    struct Foo<State: InitState = Init> {
        a: u32,
        b: DeferInit<SelfLife<u32>, State>,
        _pin: PhantomPinned,
    }

    impl Foo<Uninit> {
        pub const fn new(a: u32) -> Self {
            Self {
                a,
                b: DeferInit::uninit(),
                _pin: PhantomPinned,
            }
        }

        const unsafe fn assume_init_mut(&mut self) -> &mut Foo<Init> {
            unsafe { core::mem::transmute(self) }
        }

        pub fn init(&mut self) -> Pin<&mut Foo<Init>> {
            unsafe {
                self.b.set(SelfLife::new(&self.a));
                Pin::new_unchecked(self.assume_init_mut())
            }
        }
    }

    #[test]
    fn smoke() {
        let mut foo = Foo::new(10);
        assert_eq!(foo.a, 10);
        // assert_eq!(***foo.b, 10);
        {
            let foo = foo.init();
            assert_eq!(foo.a, 10);
            assert_eq!(***foo.b, 10);
        }
        assert_eq!(foo.a, 10);
        // assert_eq!(***foo.b, 10);
    }
}
