use core::{convert::Infallible, marker::PhantomData, ops::Deref};

use arch::{
    mem::{Addrspace, Page},
    CapabilityResource, System,
};
use derive_more::{Deref, DerefMut};
use derive_where::derive_where;
use qapi::caps::{CapError, CapIndex, CapabilityKind, PositiveIsize};
use trie::{Ptr as _, Slot, TrieEntry};

use crate::{
    kmem::KPtr,
    retyping::FrameExt,
    sync_call::{SyncCall, SyncRet},
    thread::Thread,
    PMO,
};

const SLOT_SIZE: usize = 64;
const NUM_SLOTS: usize = Page::SIZE / SLOT_SIZE;

/// A page-wide trie node for the capability tables.
pub type CapTable<S> = TrieEntry<NUM_SLOTS, CapSlot<S>>;

#[derive(Debug)]
pub struct CapSlot<S: System> {
    pub child: Option<KPtr<CapTable<S>>>,
    pub capability: Capability<S>,
}

impl<S: System> Default for CapSlot<S> {
    fn default() -> Self {
        Self {
            child: None,
            capability: Default::default(),
        }
    }
}

impl<S: System> Slot<NUM_SLOTS> for CapSlot<S> {
    type Err = Infallible;
    type Ptr<T> = KPtr<T>;

    fn child(&self) -> Result<Option<KPtr<CapTable<S>>>, Self::Err> {
        Ok(self.child.clone())
    }
}

#[derive_where(Debug, Clone)]
pub struct Resources<S: System> {
    addrspace: KPtr<S::PageTable>,
    capabilities: KPtr<CapTable<S>>,
}

#[derive(Debug, Deref)]
pub struct RefBound<'a, T> {
    #[deref]
    inner: T,
    _life: PhantomData<&'a T>,
}

impl<S: System> Resources<S> {
    pub fn new(addrspace: S::Addrspace, capabilities: KPtr<CapTable<S>>) -> Self {
        let page_table =
            unsafe { KPtr::from_frame_unchecked(addrspace.into_frame().as_kernel_unchecked()) };
        Self {
            addrspace: page_table,
            capabilities,
        }
    }

    pub fn addrspace(&self) -> impl Deref<Target = S::Addrspace> + '_ {
        // We want reference semantics here to avoid the addrspace outliving the
        // page table it refers to
        RefBound {
            inner: S::Addrspace::from_frame(PMO.get(), self.addrspace.frame()),
            _life: PhantomData,
        }
    }

    pub fn cap(&self, index: CapIndex) -> Option<impl Deref<Target = Capability<S>>> {
        let slot = CapTable::get(self.capabilities.clone(), index.value()).unwrap()?;
        Some(slot.map(|s| &s.capability))
    }
}

#[derive_where(Debug, Default, Clone)]
pub enum Capability<S: System> {
    #[derive_where(default)]
    Empty,
    Thread(KPtr<Thread<S>>),
    TranscientPageTable(KPtr<S::PageTable>),
    RootPageTable(KPtr<S::PageTable>),
    CapTable(KPtr<CapTable<S>>),
    SyncCall(SyncCall<S>),
    SyncRet(SyncRet),
    Retype(Retype),
}

impl<S: System> Capability<S> {
    pub const fn empty() -> Self {
        Self::Empty
    }

    pub fn exercise(&self, args: &[usize; 5]) -> Result<PositiveIsize, CapError> {
        match self {
            Self::Empty => Err(CapError::CapNotFound),
            Self::Thread(t) => t.exercise(args, CapabilityKind::Thread),
            Self::TranscientPageTable(p) => p.exercise(args, CapabilityKind::TranscientPageTable),
            Self::RootPageTable(p) => p.exercise(args, CapabilityKind::RootPageTable),
            Self::CapTable(c) => exercise_cap_table(c, args, CapabilityKind::CapTable),
            Self::SyncCall(s) => s.exercise(args, CapabilityKind::SyncCall),
            Self::SyncRet(s) => s.exercise(args, CapabilityKind::SyncCall),
            Self::Retype(r) => r.exercise(args, CapabilityKind::Retype),
        }
    }
}

// Don't feel like dealing with orphan rules...
fn exercise_cap_table<S: System>(
    this: &CapTable<S>,
    args: &[usize; 5],
    kind: CapabilityKind,
) -> Result<PositiveIsize, CapError> {
    todo!();
}

impl<S: System> CapabilityResource for Thread<S> {
    fn exercise(&self, args: &[usize; 5], kind: CapabilityKind) -> Result<PositiveIsize, CapError> {
        todo!()
    }
}

impl<S: System> CapabilityResource for SyncCall<S> {
    fn exercise(&self, args: &[usize; 5], kind: CapabilityKind) -> Result<PositiveIsize, CapError> {
        todo!()
    }
}

impl CapabilityResource for SyncRet {
    fn exercise(&self, args: &[usize; 5], kind: CapabilityKind) -> Result<PositiveIsize, CapError> {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct Retype;

impl CapabilityResource for Retype {
    fn exercise(&self, args: &[usize; 5], kind: CapabilityKind) -> Result<PositiveIsize, CapError> {
        todo!()
    }
}
