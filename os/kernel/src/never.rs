mod fn_traits {
    pub trait FnOnce<Args> {
        type Output;
    }

    impl<F, R> FnOnce<()> for F
    where
        F: ::core::ops::FnOnce() -> R,
    {
        type Output = R;
    }
}

pub type Never = <fn() -> ! as fn_traits::FnOnce<()>>::Output;
