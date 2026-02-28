pub mod boxed;

mod undrop {
    #[doc(hidden)]
    pub struct Helper<const D: bool>;
    impl<const D: bool> Drop for Helper<D> {
        fn drop(&mut self) {
            const {
                if !D {
                    panic!("this type cannot be dropped");
                }
            }
        }
    }

    /// A marker type which cannot be dropped.
    ///
    /// Do to how struct/enum fields are automatically
    /// dropped, placing this type at any depth will
    /// cause an implicit or explicit drop of the
    /// containing type to throw a post-mono error.
    pub type PhantomUndrop = Helper<false>;
    impl PhantomUndrop {
        pub fn drop(self) {
            let droppable: Helper<true> = unsafe { std::mem::transmute(self) };
            std::mem::drop(droppable);
        }
    }
    #[allow(non_upper_case_globals)]
    pub const PhantomUndrop: PhantomUndrop = Helper;
}
