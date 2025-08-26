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

    pub struct ExceptionArgs {
        pub kind: usize,
        pub code: u64,
        pub extra: u64,
    }

    impl SyncAbi for ExceptionAbi {
        type Args = ExceptionArgs;
        type Ret = ();
    }
}
