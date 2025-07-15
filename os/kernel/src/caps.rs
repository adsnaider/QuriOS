use core::{convert::Infallible, mem::MaybeUninit, ops::Deref};

use arch::{mem::Page, System};
use qapi::caps::{CapError, CapIndex, CapabilityKind, PositiveIsize};
use trie::{Ptr as _, Slot, TrieEntry};

use crate::{kmem::KPtr, retyping::KernelFrame};

const SLOT_SIZE: usize = 64;
const NUM_SLOTS: usize = Page::SIZE / SLOT_SIZE;

/// A page-wide trie node for the capability tables.
pub type CapTable = TrieEntry<NUM_SLOTS, CapSlot>;

#[derive(Debug, Default, Clone)]
pub struct CapSlot {
    pub child: Option<KPtr<CapTable>>,
    pub capability: Capability,
}

impl Slot<NUM_SLOTS> for CapSlot {
    type Err = Infallible;
    type Ptr<T> = KPtr<T>;

    fn child(&self) -> Result<Option<KPtr<CapTable>>, Self::Err> {
        Ok(self.child.clone())
    }
}

#[derive(Debug)]
pub struct Resources<S: System> {
    addrspace: S::Addrspace,
    capabilities: KPtr<CapTable>,
}

impl<S: System> Resources<S> {
    pub const fn new(addrspace: S::Addrspace, capabilities: KPtr<CapTable>) -> Self {
        Self {
            addrspace,
            capabilities,
        }
    }

    pub fn addrspace(&self) -> &S::Addrspace {
        &self.addrspace
    }

    pub fn cap(&self, index: CapIndex) -> Option<impl Deref<Target = Capability>> {
        let slot = CapTable::get(self.capabilities.clone(), index.value()).unwrap()?;
        Some(slot.map(|s| &s.capability))
    }
}

#[derive(Debug)]
pub struct Capability {
    pub resource: MaybeUninit<KernelFrame>,
    pub kind: CapabilityKind,
}

impl Default for Capability {
    fn default() -> Self {
        Self::empty()
    }
}

impl Clone for Capability {
    fn clone(&self) -> Self {
        let resource: MaybeUninit<KernelFrame> = self
            .resource()
            .map(|res| MaybeUninit::new(res.try_clone().unwrap()))
            .unwrap_or(MaybeUninit::uninit());
        Self {
            kind: self.kind,
            resource,
        }
    }
}

impl Capability {
    pub const fn empty() -> Self {
        Self {
            kind: CapabilityKind::Empty,
            resource: MaybeUninit::uninit(),
        }
    }

    pub const fn resource(&self) -> Option<&KernelFrame> {
        match self.kind {
            CapabilityKind::Empty => None,
            _ => {
                // SAFETY: We have a resource unless the slot is empty.
                Some(unsafe { self.resource.assume_init_ref() })
            }
        }
    }

    pub fn exercise(&self, _args: &[usize; 5]) -> Result<PositiveIsize, CapError> {
        match self.kind {
            CapabilityKind::Empty => Err(CapError::CapNotFound),
            CapabilityKind::Thread => todo!(),
            CapabilityKind::TranscientPageTable => todo!(),
            CapabilityKind::RootPageTable => todo!(),
            CapabilityKind::CapTable => todo!(),
        }
    }
}
