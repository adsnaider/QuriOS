use core::cell::{Ref, RefCell, RefMut};
use core::sync::atomic::{AtomicU16, Ordering};

use derive_more::{Display, Error, From};
use tap::TapFallible;

use super::CORE_LOCAL_CORE_ID;

const NO_AFFINITY: u16 = u16::MAX;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Affinity {
    Set(u16),
    Unset,
}

// A synchronization primitive that ties a piece of memory to a specific core.
#[derive(Debug)]
pub struct CoreCell<T> {
    payload: RefCell<T>,
    affinity: AtomicU16,
}

#[derive(Error, Display, Debug, Clone)]
pub enum SetAffinityError {
    #[display("Unable to switch core-cell affinity (tied to {affinity})")]
    Bounded { affinity: u16 },
    #[display("Unable to change core-cell affinity (already tied to this core)")]
    BoundToSelf,
}

#[derive(Error, Display, Debug, Clone)]
pub enum ResetAffinityError {
    #[display(
        "Can't reset the core-cell affinity as it's not currently tied to the running core (bound to core: {affinity})"
    )]
    Bounded { affinity: u16 },
    #[display("Can't reset the core-cell affinity as it's already unbound")]
    Unbound,
    #[display("The payload is still borrowed so the affinity may not change")]
    PayloadIsBorrowed,
}

#[derive(Error, Display, Debug, From)]
pub enum BorrowError {
    #[display("The cell is not bound to the current core")]
    NotLocallyBound,
    RefError(core::cell::BorrowError),
}

#[derive(Error, Display, Debug, From)]
pub enum BorrowMutError {
    #[display("The cell is not bound to the current core")]
    NotLocallyBound,
    RefMutError(core::cell::BorrowMutError),
}

// SAFETY: We don't provide an into-inner method but Drop may run on a different core if T: Send
unsafe impl<T: Send> Send for CoreCell<T> {}
// SAFETY: Only one thread (i.e. core in the non-preemptive kernel) will have access to &T.
unsafe impl<T: Send> Sync for CoreCell<T> {}

impl<T> CoreCell<T> {
    pub const fn new(payload: T) -> Self {
        Self {
            payload: RefCell::new(payload),
            affinity: AtomicU16::new(NO_AFFINITY),
        }
    }

    pub fn set_affinity(&self) -> Result<(), SetAffinityError>
    where
        T: core::fmt::Debug,
    {
        let core_affinity = *CORE_LOCAL_CORE_ID;
        self.affinity
            .compare_exchange(
                NO_AFFINITY,
                core_affinity,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .tap_ok(|previous| {
                log::debug!(
                    "Updated affinity of {:?} from {previous} to {core_affinity}",
                    self.payload
                )
            })
            .map_err(|prev| {
                if prev == core_affinity {
                    SetAffinityError::BoundToSelf
                } else {
                    SetAffinityError::Bounded { affinity: prev }
                }
            })?;
        Ok(())
    }

    pub fn reset_affinity(&self) -> Result<(), ResetAffinityError> {
        let core_affinity = *CORE_LOCAL_CORE_ID;
        // Why can we do this in 2 atomic operations without race conditions?
        // The fundamental realization here is that when the cell is tied to this core,
        // then no one else will be able to access it or change it behind our backs. This + the
        // nature of the non-preemptive kernel guarantees that the affinity won't be changed
        // behind our backs and no spurious references will be given out.
        match self.get_affinity() {
            Affinity::Unset => return Err(ResetAffinityError::Unbound),
            Affinity::Set(bounded) if bounded != core_affinity => {
                return Err(ResetAffinityError::Bounded { affinity: bounded });
            }
            _ => {} // Bound to this core
        }
        if self.payload.try_borrow_mut().is_ok() {
            self.affinity.store(NO_AFFINITY, Ordering::Release);
            Ok(())
        } else {
            Err(ResetAffinityError::PayloadIsBorrowed)
        }
    }

    pub fn try_borrow(&self) -> Result<Ref<'_, T>, BorrowError> {
        let core_affinity = *CORE_LOCAL_CORE_ID;
        let current_affinity = self.affinity.load(Ordering::Acquire);
        if core_affinity != current_affinity {
            return Err(BorrowError::NotLocallyBound);
        }
        Ok(self.payload.try_borrow()?)
    }

    pub fn try_borrow_mut(&self) -> Result<RefMut<'_, T>, BorrowMutError> {
        let core_affinity = *CORE_LOCAL_CORE_ID;
        let current_affinity = self.affinity.load(Ordering::Acquire);
        if core_affinity != current_affinity {
            return Err(BorrowMutError::NotLocallyBound);
        }
        Ok(self.payload.try_borrow_mut()?)
    }

    pub fn get_affinity(&self) -> Affinity {
        let affinity = self.affinity.load(Ordering::Relaxed);
        match affinity {
            NO_AFFINITY => Affinity::Unset,
            core => Affinity::Set(core),
        }
    }

    pub fn is_bound_to_local(&self) -> bool {
        self.get_affinity() == Affinity::Set(*CORE_LOCAL_CORE_ID)
    }
}
