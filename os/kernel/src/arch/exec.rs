use core::mem::MaybeUninit;

use qapi::{
    caps::PositiveIsize,
    init::{BootArgs, EntryFn},
};

pub trait ExecState: core::fmt::Debug {
    type RegCtx;

    fn new_thread(entry: usize, stack_top: usize, arg0: usize) -> Self;
    fn new_invocation(entry: usize, args: [MaybeUninit<usize>; 4]) -> Self;
    fn for_init_comp(entry_fun: EntryFn, stack_top: *const (), arg0: *const BootArgs) -> Self;
    fn save(&self, ctx: &Self::RegCtx);
    fn dispatch(&self) -> !;
    fn update_sync_ret(&self, resp: PositiveIsize);
}
