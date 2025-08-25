#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

use super::{PositiveIsize, new_cap_macro::ctype};
ctype!(
    pub struct SyncInvokeCap;
);

#[derive(Debug, Default, Copy, Clone)]
pub struct StandardAbi;
#[derive(Debug)]
pub struct StandardRetAbi;

pub trait SyncAbi {
    type Args;
    type Ret;
}

impl SyncAbi for StandardAbi {
    type Args = (usize, usize, usize, usize);
    type Ret = PositiveIsize;
}

#[cfg(target_arch = "x86_64")]
mod x86_64 {
    use super::SyncAbi;

    #[derive(Debug, Copy, Clone)]
    pub struct ExceptionAbi;

    #[derive(Debug)]
    pub struct ExceptionRetAbi;

    impl SyncAbi for ExceptionAbi {
        type Args = (usize, u64);
        type Ret = ();
    }
}
