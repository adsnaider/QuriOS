use crate::arch::x86_64_impl::X64Sys;
use crate::notify::Notification;

#[derive(Debug, Clone)]
pub struct IrqDispatchTable {
    handlers: [Option<Notification<X64Sys>>; 256],
}

pub struct IrqEntry<'a> {
    entry: u8,
    table: &'a IrqDispatchTable,
}

pub struct IrqEntryMut<'a> {
    entry: u8,
    table: &'a mut IrqDispatchTable,
}

impl IrqDispatchTable {
    pub const fn new() -> Self {
        Self {
            handlers: [const { None }; 256],
        }
    }

    pub fn get(&self, id: u8) -> IrqEntry<'_> {
        IrqEntry {
            entry: id,
            table: self,
        }
    }

    pub fn get_mut(&mut self, id: u8) -> IrqEntryMut<'_> {
        IrqEntryMut {
            entry: id,
            table: self,
        }
    }
}

impl<'a> IrqEntry<'a> {}

impl<'a> IrqEntryMut<'a> {}
