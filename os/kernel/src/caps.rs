use core::marker::PhantomData;

use qapi::caps::CapabilityKind;

use crate::{retyping::KernelFrame, thread::Thread};

#[derive(Debug)]
pub struct Resources<E, A> {
    addrspace: A,
    capabilities: [Capability; 16],
    _thread_cap: PhantomData<Thread<E, A>>,
}

pub struct CapIndex(u32);

impl<E, A> Resources<E, A> {
    pub fn addrspace(&self) -> &A {
        &self.addrspace
    }

    pub fn cap(&self, index: CapIndex) -> &Capability {
        &self.capabilities[index.0 as usize]
    }
}

#[derive(Debug)]
pub struct Capability {
    pub resource: KernelFrame,
    pub kind: CapabilityKind,
}
