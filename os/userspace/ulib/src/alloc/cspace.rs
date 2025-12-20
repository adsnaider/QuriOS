#![allow(unused)]
use core::marker::PhantomData;

use allocator_api2::alloc::Allocator;
use derive_more::{Display, Error};
use qapi::caps::SysSlot;
use qapi::caps::ctable::CTableCap;

use super::allocman::Resources;

pub struct CapNode;

#[derive(Debug, Error, Display, Clone)]
pub enum CAllocError {
    #[display("Capability tree is full and can't allocate in the current execution stack")]
    CapTreeFull,
}

pub(super) trait CSpace {
    fn alloc_cap(&mut self, resources: &mut Resources) -> Result<CapNode, CAllocError>;
    fn cap_free(&mut self, cap: CapNode);
}

pub struct CapabilityMan {
    next_cap: SysSlot,
    root: CTableCap,
}

impl CapabilityMan {
    pub fn new(caps: CTableCap) -> Self {
        Self {
            root: caps,
            next_cap: SysSlot::new(0).unwrap(),
        }
    }

    pub fn alloc_cap(&mut self, resources: &mut Resources) -> Result<CapNode, CAllocError> {
        todo!();
    }

    pub fn cap_free(&mut self, cap: CapNode) {
        todo!();
    }

    pub const fn new_starting_at(caps: CTableCap, first_free: SysSlot) -> Self {
        Self {
            next_cap: first_free,
            root: caps,
        }
    }
}

impl CSpace for CapabilityMan {
    fn alloc_cap(&mut self, resources: &mut Resources) -> Result<CapNode, CAllocError> {
        self.alloc_cap(resources)
    }

    fn cap_free(&mut self, cap: CapNode) {
        self.cap_free(cap)
    }
}
