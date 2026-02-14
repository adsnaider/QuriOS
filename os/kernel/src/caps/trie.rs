use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::ops::Deref;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU64, Ordering};

use derive_more::{Display, Error};
use derive_where::derive_where;
use qapi::caps::slotid::SlotId;
use tailcall::tailcall;

use crate::kmem::KPtr;

#[derive(Debug)]
#[repr(transparent)]
pub struct TrieBlock<const COUNT: usize, T> {
    slots: [TrieSlot<COUNT, T>; COUNT],
}

impl<const COUNT: usize, T> Default for TrieBlock<COUNT, T> {
    fn default() -> Self {
        Self::empty()
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum InternalState {
    Dead,
    Superposed { refs: u32 },
    Alive { refs: u32 },
}

impl InternalState {
    pub const STATE_BITS: u32 = 2;

    pub fn from_raw(raw: u64) -> Self {
        let state = (raw >> (u64::BITS - Self::STATE_BITS)) as u8;
        let refs = raw as u32;
        match state {
            0b00 => InternalState::Dead,
            0b10 | 0b01 => InternalState::Superposed { refs },
            0b11 => InternalState::Alive { refs },
            _ => unreachable!("Unexpected state"),
        }
    }

    pub fn into_raw(self) -> u64 {
        let (state, refs) = match self {
            InternalState::Dead => return 0b00,
            InternalState::Alive { refs } => (0b11, refs),
            InternalState::Superposed { refs } => (0b10, refs),
        };
        (state << (u64::BITS - Self::STATE_BITS)) + u64::from(refs)
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct TrieSlot<const COUNT: usize, T> {
    payload: UnsafeCell<MaybeUninit<TrieSlotPayload<COUNT, T>>>,
    state: AtomicU64,
}

// SAFETY: All of the access to the payload is guarded by the atomic state.
unsafe impl<const COUNT: usize, T: Sync> Sync for TrieSlot<COUNT, T> {}

#[derive(Debug)]
#[repr(C)]
pub enum TrieSlotPayload<const COUNT: usize, T> {
    Data(T),
    Link(KPtr<TrieBlock<COUNT, T>>),
}

impl<const COUNT: usize, T> Default for TrieSlot<COUNT, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const COUNT: usize, T> TrieSlot<COUNT, T> {
    pub const fn new() -> Self {
        Self {
            payload: UnsafeCell::new(MaybeUninit::uninit()),
            state: AtomicU64::new(0),
        }
    }
}

#[repr(transparent)]
#[derive(Debug)]
#[derive_where(Default)]
pub struct Trie<const COUNT: usize, T> {
    block: TrieBlock<COUNT, T>,
}

impl<const COUNT: usize, T> Trie<COUNT, T> {
    pub const fn block_size() -> usize {
        core::mem::size_of::<TrieBlock<COUNT, T>>()
    }

    pub const fn slot_size() -> usize {
        core::mem::size_of::<TrieSlot<COUNT, T>>()
    }

    pub fn get(this: &KPtr<Self>, id: u32) -> Option<TrieRef<COUNT, T>> {
        let id: usize = id.try_into().unwrap();
        let this = this.clone();
        // SAFETY: It's okay to cast a Trie to a TrieBlock
        let block = unsafe { this.cast() };
        TrieBlock::<COUNT, T>::get_inner(block, id)
    }

    pub fn block(&self) -> &TrieBlock<COUNT, T> {
        &self.block
    }
}

impl<const COUNT: usize, T> TrieBlock<COUNT, T> {
    pub const fn empty() -> Self {
        Self {
            slots: [const { TrieSlot::new() }; COUNT],
        }
    }
    pub fn at(this: &KPtr<Self>, id: SlotId<COUNT>) -> BlockRef<'_, COUNT, T> {
        let slot = &this.slots[id.as_usize()];
        BlockRef {
            block: this,
            slot: NonNull::from(slot),
        }
    }

    #[tailcall]
    fn get_inner(this: KPtr<Self>, id: usize) -> Option<TrieRef<COUNT, T>> {
        const {
            assert!(COUNT > 0);
            assert!(COUNT.is_power_of_two());
        };
        let idx = id % COUNT;
        let idx = SlotId::try_new(idx).unwrap();
        let id = id / COUNT;
        let slot = Self::at(&this, idx).try_get()?;
        if id == 0 {
            Some(slot)
        } else {
            match slot.deref() {
                TrieSlotPayload::Data(_) => None,
                TrieSlotPayload::Link(kptr) => Self::get_inner(kptr.clone(), id),
            }
        }
    }
}

impl<const COUNT: usize, T> BlockRef<'_, COUNT, T> {
    fn slot(&self) -> &TrieSlot<COUNT, T> {
        // SAFETY: Having the KPtr to the block guarantees read-access to the slot
        unsafe { self.slot.as_ref() }
    }

    pub fn try_get(&self) -> Option<TrieRef<COUNT, T>> {
        // SAFETY: block ptr owns the slot
        unsafe { self.slot().try_read(self.block) }
    }

    pub fn try_set(
        &self,
        payload: TrieSlotPayload<COUNT, T>,
    ) -> Result<TrieRef<COUNT, T>, TrieSetError<TrieSlotPayload<COUNT, T>>> {
        // SAFETY: block ptr owns the slot
        unsafe { self.slot().try_resurrect(self.block, payload) }
    }

    pub fn kill(&self) {
        self.slot().kill()
    }
}

pub struct BlockRef<'a, const COUNT: usize, T> {
    block: &'a KPtr<TrieBlock<COUNT, T>>,
    slot: NonNull<TrieSlot<COUNT, T>>,
}

#[derive(Debug, Display, Error)]
pub enum TrieSetError<T> {
    #[display("The trie slot isn't dead yet so it may not be set")]
    NotDead(T),
}

pub struct TrieRef<const COUNT: usize, T> {
    _block: KPtr<TrieBlock<COUNT, T>>,
    slot: NonNull<TrieSlot<COUNT, T>>,
}

impl<const COUNT: usize, T> TrieSlot<COUNT, T> {
    unsafe fn try_read(&self, block_ptr: &KPtr<TrieBlock<COUNT, T>>) -> Option<TrieRef<COUNT, T>> {
        let prev = InternalState::from_raw(self.state.fetch_add(1, Ordering::Acquire));
        match prev {
            InternalState::Dead => {
                self.state.fetch_sub(1, Ordering::Relaxed);
                None
            }
            InternalState::Superposed { refs: _ } => {
                match InternalState::from_raw(self.state.fetch_sub(1, Ordering::Release)) {
                    InternalState::Dead => {
                        unreachable!("Went from superposed state to dead while holding reference")
                    }
                    InternalState::Superposed { refs: 1 } => {
                        // We are the last ones, let's switch the state to dead
                        self.state
                            .fetch_and(0x3FFF_FFFF_FFFF_FFFF, Ordering::Relaxed);
                        None
                    }
                    InternalState::Superposed { refs: _ } => None,
                    InternalState::Alive { refs: _ } => None,
                }
            }
            InternalState::Alive { refs: _ } => Some(TrieRef {
                _block: block_ptr.clone(),
                slot: NonNull::from(self),
            }),
        }
    }
    unsafe fn try_resurrect(
        &self,
        block_ptr: &KPtr<TrieBlock<COUNT, T>>,
        value: TrieSlotPayload<COUNT, T>,
    ) -> Result<TrieRef<COUNT, T>, TrieSetError<TrieSlotPayload<COUNT, T>>> {
        match self.state.compare_exchange(
            0,
            (InternalState::Superposed { refs: 1 }).into_raw(),
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => {}
            Err(_) => return Err(TrieSetError::NotDead(value)),
        }

        // SAFETY: Compare exchange guarantees proper access
        unsafe {
            self.payload.get().replace(MaybeUninit::new(value));
        };
        self.state
            .fetch_or(0xC000_0000_0000_0000, Ordering::Release);
        Ok(TrieRef {
            _block: block_ptr.clone(),
            slot: NonNull::from(self),
        })
    }

    fn kill(&self) {
        self.state
            .fetch_and(0x7FFF_FFFF_FFFF_FFFF, Ordering::Relaxed);
    }
}

impl<const COUNT: usize, T> Deref for TrieRef<COUNT, T> {
    type Target = TrieSlotPayload<COUNT, T>;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Guarantees on construction and drop mean that having a TrieRef guarantees
        // shared references to the slot are okay for the lifetime of TrieRef and the value is
        // set
        unsafe { (*self.slot.as_ref().payload.get()).assume_init_ref() }
    }
}

impl<const COUNT: usize, T> TrieRef<COUNT, T> {
    pub fn data(&self) -> Option<&T> {
        // SAFETY: We have read-access to the entry
        match unsafe { (*self.slot().payload.get()).assume_init_ref() } {
            TrieSlotPayload::Data(data) => Some(data),
            TrieSlotPayload::Link(_kptr) => None,
        }
    }

    pub fn payload(&self) -> &TrieSlotPayload<COUNT, T> {
        // SAFETY: If we have a ref then the payload must be initiailized
        unsafe { (*self.slot().payload.get()).assume_init_ref() }
    }

    fn slot(&self) -> &TrieSlot<COUNT, T> {
        // SAFETY: The ownership of a TrieRef guarantees read-access to the slot.
        unsafe { self.slot.as_ref() }
    }
}

impl<const COUNT: usize, T> Drop for TrieRef<COUNT, T> {
    fn drop(&mut self) {
        let slot = self.slot();
        let state = slot.state.fetch_sub(1, Ordering::Release);
        let state = InternalState::from_raw(state);
        match state {
            InternalState::Dead => {
                unreachable!("Unexpectedly dead slot before reference was released")
            }
            InternalState::Superposed { refs: 1 } => {
                // We are the last ones, let's switch the state to dead
                slot.state
                    .fetch_and(0x3FFF_FFFF_FFFF_FFFF, Ordering::Relaxed);
            }
            InternalState::Superposed { .. } => {} // Nothing to do here, we are just passing,
            InternalState::Alive { .. } => {}      // Nothing to do here, just passing,
        }
    }
}
