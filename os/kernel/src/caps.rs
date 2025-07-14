use core::marker::PhantomData;

use arch::System;
use qapi::caps::{CapIndex, CapabilityKind};

use crate::{retyping::KernelFrame, thread::Thread};

#[derive(Debug)]
pub struct Resources<S: System> {
    addrspace: S::Addrspace,
    capabilities: [Capability; 16],
    _thread_cap: PhantomData<Thread<S>>,
}

impl<S: System> Resources<S> {
    pub fn addrspace(&self) -> &S::Addrspace {
        &self.addrspace
    }

    pub fn cap(&self, index: CapIndex) -> &Capability {
        &self.capabilities[index.value() as usize]
    }
}

#[derive(Debug)]
pub struct Capability {
    pub resource: KernelFrame,
    pub kind: CapabilityKind,
}

impl Capability {
    pub fn exercise(&self) -> usize {
        todo!();
    }
}
