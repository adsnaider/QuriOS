#![allow(unused)]

#[cold]
#[inline(always)]
pub fn cold() {}

#[inline(always)]
pub fn likely(b: bool) -> bool {
    if !b {
        cold();
    }
    b
}

#[inline(always)]
pub fn unlikely(b: bool) -> bool {
    if b {
        cold();
    }
    b
}

macro_rules! chilly {
    ($e:expr) => {{
        crate::hint::cold();
        $e
    }};
}
pub(crate) use chilly;
