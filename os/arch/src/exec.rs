use qapi::init::{BootArgs, EntryFn};

pub trait ExecState: Clone {
    fn for_entry(entry_fun: EntryFn, args: BootArgs) -> Self;
    fn save(&self);
    fn dispatch(&self) -> !;
}
