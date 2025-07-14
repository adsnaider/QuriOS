use core::mem::MaybeUninit;

use arch::System;
use qapi::caps::{CapError, CapIndex, CapabilityKind, PositiveIsize};

use crate::retyping::KernelFrame;

#[derive(Debug)]
pub struct Resources<S: System> {
    addrspace: S::Addrspace,
    capabilities: [Capability; 16],
}

impl<S: System> Resources<S> {
    pub const fn new(addrspace: S::Addrspace) -> Self {
        Self {
            addrspace,
            capabilities: [const { Capability::empty() }; 16],
        }
    }
    pub fn addrspace(&self) -> &S::Addrspace {
        &self.addrspace
    }

    pub fn cap(&self, index: CapIndex) -> &Capability {
        &self.capabilities[index.value() as usize]
    }
}

#[derive(Debug)]
pub struct Capability {
    pub resource: MaybeUninit<KernelFrame>,
    pub kind: CapabilityKind,
}

impl Capability {
    pub const fn empty() -> Self {
        Self {
            kind: CapabilityKind::Empty,
            resource: MaybeUninit::uninit(),
        }
    }

    pub fn exercise(&self, _args: &[usize; 5]) -> Result<PositiveIsize, CapError> {
        match self.kind {
            CapabilityKind::Empty => Err(CapError::CapSlotIsEmpty),
            CapabilityKind::Thread => todo!(),
            CapabilityKind::TranscientPageTable => todo!(),
            CapabilityKind::RootPageTable => todo!(),
            CapabilityKind::CapTable => todo!(),
        }
    }
}
