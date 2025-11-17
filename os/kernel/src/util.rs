use cfg_if::cfg_if;

pub trait OptionExt {
    type T;
    unsafe fn unwrap_debug(self) -> Self::T;
}

pub trait ResultExt {
    type T;
    type E;

    unsafe fn unwrap_debug(self) -> Self::T
    where
        Self::E: core::fmt::Debug;
}

impl<T, E> ResultExt for Result<T, E> {
    type T = T;
    type E = E;

    unsafe fn unwrap_debug(self) -> Self::T
    where
        Self::E: core::fmt::Debug,
    {
        cfg_if! {
            if #[cfg(debug_assertions)] {
                self.unwrap()
            } else {
                // SAFETY: Precondition
                unsafe {
                    self.unwrap_unchecked()
                }
            }
        }
    }
}

impl<T> OptionExt for Option<T> {
    type T = T;

    unsafe fn unwrap_debug(self) -> Self::T {
        cfg_if! {
            if #[cfg(debug_assertions)] {
                self.unwrap()
            } else {
                // SAFETY: Precondition
                unsafe {
                    self.unwrap_unchecked()
                }
            }
        }
    }
}
