pub mod trie;

use core::{convert::Infallible, marker::PhantomData, mem::MaybeUninit, ops::Deref};

use crate::{
    arch::{
        mem::{phys::BadAddress, user_buffer_read, Addrspace, VirtAddr},
        System,
    },
    retyping::AsUnusedKernelError,
};
use derive_more::Deref;
use derive_where::derive_where;
use extend::ext;
use qapi::{
    caps::{CapError, NUM_SLOTS},
    types::UserPtr,
};
use trie::{Trie, TrieBlock, TrieRef, TrieSetError};
use zerocopy::{FromBytes, Immutable, KnownLayout};

use crate::{
    kmem::KPtr,
    retyping::FrameExt,
    sync_call::{SyncCall, SyncRet},
    thread::Thread,
    PMO,
};

const TRIE_SIZE: usize = NUM_SLOTS;

pub type CapTable<S> = Trie<TRIE_SIZE, Capability<S>>;
pub type CapBlock<S> = TrieBlock<TRIE_SIZE, Capability<S>>;
pub type CapRef<S> = TrieRef<TRIE_SIZE, Capability<S>>;

impl<S: System> CapRef<S> {
    pub fn cap(&self) -> Option<&Capability<S>> {
        self.data()
    }

    pub fn as_ctable(&self) -> Result<&KPtr<CapBlock<S>>, CapError> {
        let data = self.data().ok_or(CapError::CapNotFound)?;
        match data {
            Capability::CapBlock(kptr) => Ok(kptr),
            _ => Err(CapError::InvalidArg),
        }
    }

    pub fn as_addrspace(&self) -> Result<&KPtr<S::PageTable>, CapError> {
        let data = self.data().ok_or(CapError::CapNotFound)?;
        match data {
            Capability::Addrspace(kptr) => Ok(kptr),
            _ => Err(CapError::InvalidArg),
        }
    }

    pub fn as_thread(&self) -> Result<&KPtr<Thread<S>>, CapError> {
        let data = self.data().ok_or(CapError::CapNotFound)?;
        match data {
            Capability::Thread(kptr) => Ok(kptr),
            _ => Err(CapError::InvalidArg),
        }
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

    pub fn from_parts(addrspace: KPtr<S::PageTable>, capabilities: KPtr<CapTable<S>>) -> Self {
        Self {
            addrspace,
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

    pub fn cap_table(&self) -> &KPtr<CapTable<S>> {
        &self.capabilities
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
    CapBlock(KPtr<CapBlock<S>>),
    SyncCall(SyncCall<S>),
    SyncRet(SyncRet),
}

impl<S: System> Capability<S> {
    pub const fn empty() -> Self {
        Self::Empty
    }
}

#[ext]
pub impl<T> UserPtr<T> {
    fn verify(self) -> Result<TrustedUserPtr<T>, CapError> {
        TrustedUserPtr::try_from(self)
    }
}

pub struct TrustedUserPtr<T>(*mut T);
impl<T> TryFrom<UserPtr<T>> for TrustedUserPtr<T> {
    type Error = CapError;

    fn try_from(value: UserPtr<T>) -> Result<Self, Self::Error> {
        let start = value.addr();
        let end = value
            .addr()
            .checked_add(size_of::<T>())
            .ok_or(CapError::BadUserMemory)?;
        let start_ptr = VirtAddr::try_new(start).map_err(|_| CapError::BadUserMemory)?;
        let end_ptr = VirtAddr::try_new(end).map_err(|_| CapError::BadUserMemory)?;
        if !start_ptr.is_user() || !end_ptr.is_user() {
            return Err(CapError::BadUserMemory);
        }
        Ok(Self(start as *mut T))
    }
}

impl<T> TrustedUserPtr<T>
where
    T: FromBytes + KnownLayout + Immutable + Copy,
{
    pub fn safe_read(&self) -> Result<T, CapError> {
        let mut result = MaybeUninit::<T>::uninit();
        // SAFETY: TrustedUserPtr construction verifies the validity of the pointer itself and the routine guarantees proper
        // handling of page fault to return false.
        let success = unsafe {
            user_buffer_read(
                result.as_mut_ptr() as *mut u8,
                self.0 as *const u8,
                size_of::<T>(),
            )
        };
        if success {
            // SAFETY: `user_buffer_read` guarantees that true is returned if the copy was
            // successful. Since T implements FromBytes + KnownLayout + Imuutable, we have
            // guarantees that the only failure cases are 1. misaligned pointer (checked at
            // construction), or size error (assumed at construction).
            Ok(unsafe { result.assume_init() })
        } else {
            Err(CapError::BadUserMemory)
        }
    }
}

impl From<BadAddress> for CapError {
    fn from(_: BadAddress) -> Self {
        Self::InvalidArg
    }
}

impl From<AsUnusedKernelError> for CapError {
    fn from(value: AsUnusedKernelError) -> Self {
        match value {
            AsUnusedKernelError::NotExpectedState(_) => CapError::NotKernelTyped,
            AsUnusedKernelError::AlreadyInUse => CapError::FrameInUse,
            AsUnusedKernelError::OutOfBounds(_) => CapError::InvalidArg,
        }
    }
}

impl<T> From<TrieSetError<T>> for CapError {
    fn from(value: TrieSetError<T>) -> Self {
        match value {
            TrieSetError::NotDead(_) => CapError::CapNotEmpty,
        }
    }
}
