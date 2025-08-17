use core::marker::PhantomData;

use allocator_api2::alloc::Allocator;
use derive_more::{Display, Error};
use qapi::caps::SysSlot;
use qapi::caps::ctable::CapTableCap;

pub struct CapNode;

#[derive(Debug, Error, Display, Clone)]
pub enum CAllocError {}

pub trait CapAlloc {
    fn alloc_cap(&mut self) -> Result<CapNode, CAllocError>;
    fn cap_free(&mut self, cap: CapNode);
}

pub struct CapabilityMan<A: Allocator> {
    next_cap: SysSlot,
    root: CapTableCap,
    _alloc: PhantomData<A>,
}

impl<A: Allocator> CapabilityMan<A> {
    pub fn new(caps: CapTableCap, allocator: A) -> Self {
        Self {
            root: caps,
            next_cap: SysSlot::new(0).unwrap(),
            _alloc: PhantomData,
        }
    }

    pub fn alloc_cap(&mut self) -> Result<CapNode, CAllocError> {
        todo!();
    }

    pub fn cap_free(&mut self, cap: CapNode) {
        todo!();
    }

    pub const fn new_starting_at(caps: CapTableCap, first_free: SysSlot) -> Self {
        Self {
            next_cap: first_free,
            root: caps,
            _alloc: PhantomData,
        }
    }

    pub fn allocator(&self) -> &A {
        todo!();
    }
}
