use qapi::init::{BootArgs, EntryFn};

pub trait ExecState: Clone + core::fmt::Debug {
    fn for_entry(entry_fun: EntryFn, stack_top: *const (), arg0: *const BootArgs) -> Self;
    fn save(&self);
    fn dispatch(&self) -> !;
}
