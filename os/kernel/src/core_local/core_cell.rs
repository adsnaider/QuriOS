use core::cell::{Cell, UnsafeCell};
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU16, Ordering};

use derive_more::{Display, Error};
use qapi::caps::CapError;
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
    payload: T,
    refs: Cell<usize>,
    affinity: AtomicU16,
}

#[derive(Error, Display, Debug, Clone)]
pub enum BindError {
    #[display("Unable to switch core-cell affinity (tied to another core: {affinity})")]
    Bound { affinity: u16 },
}

#[derive(Error, Display, Debug, Clone)]
pub enum GetError {
    #[display("Unable to get the payload since the cell is not locally bound")]
    Unbound,
}

#[derive(Error, Display, Debug, Clone)]
pub enum UnbindError {
    #[display(
        "Can't reset the core-cell affinity as it's not currently tied to the running core (bound to core: {affinity})"
    )]
    Bound { affinity: u16 },
    #[display("There are still {refs} references to this cell alive")]
    RefsAlive { refs: usize },
}

// SAFETY: We don't provide an into-inner method but Drop may run on a different core if T: Send
unsafe impl<T: Send> Send for CoreCell<T> {}
// SAFETY: Only one thread (i.e. core in the non-preemptive kernel) will have access to &T.
unsafe impl<T: Send> Sync for CoreCell<T> {}

pub struct CoreGuard<'a, T> {
    cell: &'a CoreCell<T>,
    _not_thread_safe: PhantomData<*mut T>,
}

impl<'a, T> CoreGuard<'a, T> {
    pub fn new(cell: &'a CoreCell<T>) -> Result<Self, GetError> {
        if cell.is_locally_bound() {
            cell.refs.update(|c| c + 1);
            Ok(Self {
                cell,
                _not_thread_safe: PhantomData,
            })
        } else {
            Err(GetError::Unbound)
        }
    }
}

impl<T> Drop for CoreGuard<'_, T> {
    fn drop(&mut self) {
        let count = self.cell.refs.get();
        assert!(count >= 1);
        self.cell.refs.set(count - 1);
    }
}

impl<T> Deref for CoreGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.cell.payload
    }
}

impl<T> CoreCell<T> {
    pub const fn new(payload: T) -> Self {
        Self {
            payload,
            refs: Cell::new(0),
            affinity: AtomicU16::new(NO_AFFINITY),
        }
    }

    pub fn bind(&self) -> Result<CoreGuard<'_, T>, BindError> {
        let core_affinity = *CORE_LOCAL_CORE_ID;
        self.affinity
            .compare_exchange(
                NO_AFFINITY,
                core_affinity,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .tap_ok(|previous| {
                log::debug!("Updated affinity of payload from {previous} to {core_affinity}",)
            })
            .map(|_| ())
            .or_else(|prev| {
                if prev == core_affinity {
                    Ok(())
                } else {
                    Err(BindError::Bound { affinity: prev })
                }
            })?;
        assert_eq!(self.refs.get(), 0);
        Ok(CoreGuard::new(self).unwrap())
    }

    pub fn unbind(&self) -> Result<(), UnbindError> {
        let core_affinity = *CORE_LOCAL_CORE_ID;
        // Why can we do this in 2 atomic operations without race conditions?
        // The fundamental realization here is that when the cell is tied to this core,
        // then no one else will be able to access it or change it behind our backs. This + the
        // nature of the non-preemptive kernel guarantees that the affinity won't be changed
        // behind our backs and no spurious references will be given out.
        match self.current_bind() {
            Affinity::Unset => Ok(()),
            Affinity::Set(bounded) if bounded != core_affinity => {
                Err(UnbindError::Bound { affinity: bounded })
            }
            _ => {
                // Bound to this core
                let refs = self.refs.get();
                if refs == 0 {
                    self.affinity.store(NO_AFFINITY, Ordering::Release);
                    Ok(())
                } else {
                    Err(UnbindError::RefsAlive { refs })
                }
            }
        }
    }

    pub fn try_get(&self) -> Result<CoreGuard<T>, GetError> {
        CoreGuard::new(self)
    }

    pub fn do_bound<F, U>(&self, fun: F) -> Result<U, BindError>
    where
        F: FnOnce(CoreGuard<T>) -> U,
    {
        let guard = self.bind()?;
        let out = Ok(fun(guard));
        self.unbind().unwrap();
        out
    }

    pub fn with<F, U>(&self, fun: F) -> Result<U, GetError>
    where
        F: FnOnce(&T) -> U,
    {
        let guard = CoreGuard::new(self)?;
        Ok(fun(&guard))
    }

    pub fn current_bind(&self) -> Affinity {
        let affinity = self.affinity.load(Ordering::Relaxed);
        match affinity {
            NO_AFFINITY => Affinity::Unset,
            core => Affinity::Set(core),
        }
    }

    pub fn is_locally_bound(&self) -> bool {
        self.current_bind() == Affinity::Set(*CORE_LOCAL_CORE_ID)
    }
}

#[derive(Debug, Error, Display)]
pub enum LockError {
    #[display("The payload is currently locked")]
    AlreadyLocked,
}

#[derive(Debug)]
pub struct Lock<T> {
    inner: UnsafeCell<T>,
    locked: Cell<bool>,
}

impl<T> Lock<T> {
    pub const fn new(inner: T) -> Self {
        Self {
            inner: UnsafeCell::new(inner),
            locked: Cell::new(false),
        }
    }

    pub fn lock(&self) -> LockGuard<'_, T> {
        LockGuard::new(self).unwrap()
    }

    pub fn try_lock(&self) -> Result<LockGuard<'_, T>, LockError> {
        LockGuard::new(self)
    }

    pub fn try_locked<F, U>(&self, fun: F) -> Result<U, LockError>
    where
        F: FnOnce(&mut T) -> U,
    {
        let mut guard = LockGuard::new(self)?;
        Ok(fun(&mut *guard))
    }

    pub fn locked<F, U>(&self, fun: F) -> U
    where
        F: FnOnce(&mut T) -> U,
    {
        let mut guard = LockGuard::new(self).unwrap();
        fun(&mut *guard)
    }

    pub fn replace(&self, new: T) -> Result<T, LockError> {
        let mut guard = LockGuard::new(self)?;
        let prev = core::mem::replace(&mut *guard, new);
        Ok(prev)
    }
}

pub struct LockGuard<'a, T> {
    cell: &'a Lock<T>,
    _not_thread_safe: PhantomData<*mut T>,
}

impl<'a, T> LockGuard<'a, T> {
    pub fn new(cell: &'a Lock<T>) -> Result<Self, LockError> {
        if cell.locked.get() {
            return Err(LockError::AlreadyLocked);
        }
        cell.locked.set(true);
        let guard = Self {
            cell,
            _not_thread_safe: PhantomData,
        };
        Ok(guard)
    }
}

impl<T> Deref for LockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Lockguard guarantees exclusive access
        unsafe { &*self.cell.inner.get() }
    }
}

impl<T> DerefMut for LockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: Lockguard guarantees exclusive access
        unsafe { &mut *self.cell.inner.get() }
    }
}

impl<T> Drop for LockGuard<'_, T> {
    fn drop(&mut self) {
        self.cell.locked.set(false);
    }
}

impl From<BindError> for CapError {
    fn from(value: BindError) -> Self {
        match value {
            BindError::Bound { affinity: _ } => CapError::ThreadBoundToOtherCore,
        }
    }
}
