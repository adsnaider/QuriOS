use qapi::init::{BootArgs, EntryFn};

pub trait ExecState: core::fmt::Debug {
    fn new_thread(entry: usize, stack_top: usize) -> Self;
    fn for_entry(entry_fun: EntryFn, stack_top: *const (), arg0: *const BootArgs) -> Self;
    fn save(&self);
    fn dispatch(&self) -> !;
}
