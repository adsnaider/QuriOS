use core::{convert::Infallible, marker::PhantomData, mem::MaybeUninit, ops::Deref, ptr::NonNull};

use arch::{
    mem::{user_buffer_read, Addrspace, Page, VirtAddr},
    system, ArchSystem, CapabilityResource, System,
};
use derive_more::{Deref, DerefMut};
use derive_where::derive_where;
use qapi::{
    caps::{
        cap_table::{CapTableOps, ConsKind, ConsOp, ThreadCons},
        CapError, CapId, CapabilityKind, PositiveIsize, NUM_SLOTS, SLOT_SIZE,
    },
    syscall::{SyscallArgs, SyscallArgsInit, SyscallStruct},
    types::UserPtr,
};
use sync::cell::AtomicCell;
use trie::{Ptr, Slot, TrieEntry};
use zerocopy::{AlignmentError, CastError, FromBytes, Immutable, KnownLayout};

use crate::{
    kmem::KPtr,
    retyping::FrameExt,
    sync_call::{SyncCall, SyncRet},
    thread::Thread,
    PMO,
};

/// A page-wide trie node for the capability tables.
pub type CapTable<S> = TrieEntry<NUM_SLOTS, CapSlot<S>>;

#[derive(Debug)]
#[derive_where(Clone)]
pub struct ImmutableSlot<S: System> {
    pub child: Option<KPtr<CapTable<S>>>,
    pub capability: Capability<S>,
}

#[derive(Deref, DerefMut, Debug)]
#[repr(align(128))]
pub struct CapSlot<S: System>(AtomicCell<ImmutableSlot<S>>);

impl<S: System> Default for CapSlot<S> {
    fn default() -> Self {
        const {
            assert!(CapTable::<S>::slot_size() == SLOT_SIZE);
            assert!(core::mem::size_of::<CapTable<S>>() == Page::SIZE);
        }
        Self(AtomicCell::new(ImmutableSlot::default()))
    }
}

impl<S: System> Default for ImmutableSlot<S> {
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
        Ok(self.get_cloned().child)
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
        // SAFETY: Addrspace frame is already a kenrel frame by construction and holds a PageTable type.
        let page_table =
            unsafe { KPtr::from_frame_unchecked(addrspace.into_frame().as_kernel_unchecked()) };
        Self {
            addrspace: page_table,
            capabilities,
        }
    }

    pub fn addrspace_cap(&self) -> &KPtr<S::PageTable> {
        &self.addrspace
    }

    pub fn addrspace(&self) -> impl Deref<Target = S::Addrspace> + '_ {
        // We want reference semantics here to avoid the addrspace outliving the
        // page table it refers to
        RefBound {
            inner: S::Addrspace::from_frame(PMO.get(), self.addrspace.frame()),
            _life: PhantomData,
        }
    }

    pub fn cap(&self, index: CapId) -> Option<Capability<S>> {
        let slot = self.slot(index)?;
        Some(slot.get_cloned().capability)
    }

    pub fn slot(&self, index: CapId) -> Option<impl Ptr<CapSlot<S>>> {
        CapTable::get(self.capabilities.clone(), index.value()).unwrap_infallible()
    }
}

pub trait UnwrapInfallible<T> {
    fn unwrap_infallible(self) -> T;
}

impl<T> UnwrapInfallible<T> for Result<T, Infallible> {
    fn unwrap_infallible(self) -> T {
        self.unwrap()
    }
}

#[derive_where(Debug, Default, Clone)]
pub enum Capability<S: System> {
    #[derive_where(default)]
    Empty,
    Thread(KPtr<Thread<S>>),
    TranscientPageTable(KPtr<S::PageTable>),
    Addrspace(KPtr<S::PageTable>),
    CapTable(KPtr<CapTable<S>>),
    SyncCall(SyncCall<S>),
    SyncRet(SyncRet),
    Retype(Retype),
}

impl<S: System> Capability<S> {
    pub const fn empty() -> Self {
        Self::Empty
    }

    pub fn exercise(&self, args: SyscallArgs<SyscallArgsInit>) -> Result<PositiveIsize, CapError> {
        match self {
            Self::Empty => Err(CapError::CapNotFound),
            Self::Thread(t) => t.exercise(args, CapabilityKind::Thread),
            Self::TranscientPageTable(p) => p.exercise(args, CapabilityKind::TranscientPageTable),
            Self::Addrspace(p) => p.exercise(args, CapabilityKind::RootPageTable),
            Self::CapTable(c) => exercise_cap_table(c, args, CapabilityKind::CapTable),
            Self::SyncCall(s) => s.exercise(args, CapabilityKind::SyncCall),
            Self::SyncRet(s) => s.exercise(args, CapabilityKind::SyncCall),
            Self::Retype(r) => r.exercise(args, CapabilityKind::Retype),
        }
    }
}

pub struct TrustedUserPtr<T>(NonNull<T>);
impl<T> TryFrom<UserPtr<T>> for TrustedUserPtr<T> {
    type Error = CapError;

    fn try_from(value: UserPtr<T>) -> Result<Self, Self::Error> {
        let start = value.addr();
        if start == 0 {
            return Err(CapError::BadUserMemory);
        }
        let end = value
            .addr()
            .checked_add(size_of::<T>())
            .ok_or(CapError::BadUserMemory)?;
        let start_ptr = VirtAddr::try_new(start).map_err(|_| CapError::BadUserMemory)?;
        let end_ptr = VirtAddr::try_new(end).map_err(|_| CapError::BadUserMemory)?;
        if !start_ptr.is_user() || !end_ptr.is_user() {
            return Err(CapError::BadUserMemory);
        }
        Ok(Self(NonNull::new(start as *mut T).unwrap()))
    }
}

impl<T> TrustedUserPtr<T>
where
    T: FromBytes + KnownLayout + Immutable + Copy,
{
    pub fn safe_read(&self) -> Result<T, CapError> {
        let mut result = MaybeUninit::<T>::uninit();
        let success = unsafe {
            user_buffer_read(
                result.as_mut_ptr() as *mut u8,
                self.0.as_ptr() as *const u8,
                size_of::<T>(),
            )
        };
        if success {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(CapError::BadUserMemory)
        }
    }
}

// Don't feel like dealing with orphan rules...
fn exercise_cap_table<S: System>(
    this: &KPtr<CapTable<S>>,
    args: SyscallArgs<SyscallArgsInit>,
    kind: CapabilityKind,
) -> Result<PositiveIsize, CapError> {
    match CapTableOps::try_from_args(args)? {
        CapTableOps::Cons(ConsOp {
            slot_id,
            kind,
            cons_args,
        }) => {
            let slot = CapTable::index(this.clone(), slot_id);
            match kind {
                ConsKind::Thread => {
                    let cons_args = TrustedUserPtr::try_from(cons_args.cast::<ThreadCons>())?;
                    let args = cons_args.safe_read()?;

                    todo!();
                }
                ConsKind::CapTable => todo!(),
                ConsKind::TranscientPageTable => todo!(),
                ConsKind::Addrspace => todo!(),
                ConsKind::SyncCall => todo!(),
                ConsKind::SyncRet => todo!(),
            }
        }
    }
}

impl<S: System> CapabilityResource for Thread<S> {
    fn exercise(
        &self,
        args: SyscallArgs<SyscallArgsInit>,
        kind: CapabilityKind,
    ) -> Result<PositiveIsize, CapError> {
        todo!();
    }
}

impl<S: System> CapabilityResource for SyncCall<S> {
    fn exercise(
        &self,
        args: SyscallArgs<SyscallArgsInit>,
        kind: CapabilityKind,
    ) -> Result<PositiveIsize, CapError> {
        todo!()
    }
}

impl CapabilityResource for SyncRet {
    fn exercise(
        &self,
        args: SyscallArgs<SyscallArgsInit>,
        kind: CapabilityKind,
    ) -> Result<PositiveIsize, CapError> {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct Retype;

impl CapabilityResource for Retype {
    fn exercise(
        &self,
        args: SyscallArgs<SyscallArgsInit>,
        kind: CapabilityKind,
    ) -> Result<PositiveIsize, CapError> {
        todo!()
    }
}
