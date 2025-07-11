mod bump_alloc;

use core::mem::{ManuallyDrop, MaybeUninit};
use core::sync::atomic::{AtomicU16, Ordering};

use arch::mem::{Frame, Page, VirtAddr};
use derive_more::{Display, Error, From};
use limine::memory_map::{Entry, EntryType};
use sync::cell::{AtomicOnceCell, OnceError};

use crate::pmo::{PhysAddrExt as _, VirtAddrExt as _};
use crate::retyping::bump_alloc::BumpAllocator;

pub type MemoryMap = &'static mut [&'static mut Entry];
static RETYPE_TABLE: AtomicOnceCell<RetypeTable> = AtomicOnceCell::new();

pub struct RetypeTable {
    retype_map: &'static mut [RetypeEntry],
}

pub struct RetypeMetadata<I: IntoIterator<Item = Frame>> {
    /// The actual pointer and length that encompases the retype table.
    pub map: (*const RetypeEntry, usize),
    /// An iterator over the frames that make up the map.
    pub frames: I,
}

#[derive(Debug, Error, Display, From)]
pub enum RetypeInitError {
    DoubleInitialization(#[from] OnceError),
    #[display("The memory map provided is completely empty")]
    MemoryMapEmpty,
    #[display("Insufficient memory in the system to bootsrap the retype table")]
    OutOfMemory,
}

impl RetypeTable {
    pub fn memory_map_meta() -> RetypeMetadata<impl IntoIterator<Item = Frame>> {
        let mem = RETYPE_TABLE.get().unwrap().retype_map.as_ptr_range();
        // SAFETY: HHDM was used to construct the retype table originally.
        let start =
            Frame::from_start_address(unsafe { VirtAddr::new(mem.start.addr()).to_physical() });
        // SAFETY: HHDM was used to construct the retype table originally.
        let end = Frame::within_frame(unsafe { VirtAddr::new(mem.end.addr()).to_physical() });
        let iter = core::iter::successors(Some(start), move |prev| {
            let next = prev.next();
            if next.addr().as_u64() > end.addr().as_u64() {
                None
            } else {
                Some(next)
            }
        });
        let bytes = mem.end.addr() - mem.start.addr();
        let count = bytes / core::mem::size_of::<RetypeEntry>();
        assert!(bytes % core::mem::size_of::<RetypeEntry>() == 0);
        RetypeMetadata {
            map: (mem.start, count),
            frames: iter,
        }
    }

    /// # Safety
    ///
    /// The memory map must accurately describe the system's memory
    pub unsafe fn new(memory_map: MemoryMap) -> Result<Self, RetypeInitError> {
        let active_memory_top = {
            let last = memory_map
                .iter()
                .rev()
                .find(|entry| entry.entry_type != EntryType::RESERVED)
                .ok_or(RetypeInitError::MemoryMapEmpty)?;
            last.base + last.length
        };
        log::debug!("Initializing the retype table");
        assert!(active_memory_top % Frame::SIZE == 0);
        let number_frames = (active_memory_top / Frame::SIZE) as usize;
        let mut allocator = BumpAllocator::new(memory_map);

        let retype_map_frames = {
            let retype_map_size = core::mem::size_of::<RetypeEntry>() * number_frames;
            assert!(retype_map_size > 0);
            retype_map_size.div_ceil(Page::SIZE)
        };
        log::debug!("Frames required: {retype_map_frames}");
        let retype_map: &mut [MaybeUninit<RetypeEntry>] = {
            let start_physical_address = allocator
                .alloc_frames(retype_map_frames)
                .ok_or(RetypeInitError::OutOfMemory)?;
            let start_addr: *mut MaybeUninit<RetypeEntry> =
                start_physical_address.to_virtual().as_mut_ptr();
            // SAFETY: Memory is allocated and off the memory map
            unsafe { core::slice::from_raw_parts_mut(start_addr, number_frames) }
        };

        for entry in retype_map.iter_mut() {
            entry.write(RetypeEntry::unavailable());
        }
        // SAFETY: Initialized in earlier loop
        let retype_map: &mut [RetypeEntry] = unsafe { core::mem::transmute(retype_map) };

        let memory_map = allocator.into_memory_map();
        for entry in memory_map.iter() {
            assert!(entry.base % Frame::SIZE == 0);
            assert!(entry.length % Frame::SIZE == 0);
            let start_idx = (entry.base / Frame::SIZE) as usize;
            let count = (entry.length / Frame::SIZE) as usize;
            for slot in retype_map.iter_mut().skip(start_idx).take(count) {
                let retype_entry = match entry.entry_type {
                    EntryType::USABLE => RetypeEntry::untyped(),
                    EntryType::BOOTLOADER_RECLAIMABLE | EntryType::EXECUTABLE_AND_MODULES => {
                        RetypeEntry::kernel(1)
                    }
                    _ => RetypeEntry::unavailable(),
                };
                *slot = retype_entry;
            }
        }
        Ok(Self { retype_map })
    }
}

/// # Safety
///
/// The memory map must accurately describe the system's memory
pub unsafe fn init(memory_map: MemoryMap) -> Result<(), RetypeInitError> {
    // SAFETY: Precondition
    RETYPE_TABLE.set(unsafe { RetypeTable::new(memory_map) }?)?;
    Ok(())
}

#[derive(Debug, Display, Error)]
#[display("Invalid retype entry is past the end of memory")]
pub struct OutOfBounds;

#[derive(Debug, From, Display, Error)]
pub enum RetypeError {
    #[display("Invalid start state {_0:?}")]
    InvalidFromState(#[error(not(source))] State),
    #[display("Couldn't retype due to non-zero references")]
    RefsExist(#[error(not(source))] u16),
    OutOfBounds(#[from] OutOfBounds),
}

#[derive(Debug, Display, From, Error)]
pub enum AsTypeError {
    #[display("Invalid start state {_0:?}")]
    NotExpectedState(#[error(not(source))] State),
    MaxRefs(#[from] MaxRefs),
    OutOfBounds(#[from] OutOfBounds),
}

#[derive(Debug, From, Display, Error)]
pub enum AsUnusedKernelError {
    #[display("Invalid start state for kernel: {_0:?}")]
    NotExpectedState(#[error(not(source))] State),
    #[display("Kernel frame is already used")]
    AlreadyInUse,
    OutOfBounds(#[from] OutOfBounds),
}

#[derive(Debug, Copy, Clone, Display, Error)]
#[display("Reached maximum reference count limit")]
pub struct MaxRefs;
#[derive(Debug, Copy, Clone)]
pub struct NoRefs;

#[repr(transparent)]
#[derive(Debug)]
pub struct UserFrame(Frame);

#[extend::ext]
pub impl Frame {
    fn memory_limit() -> usize {
        let nframes = RETYPE_TABLE.get().unwrap().retype_map.len();
        nframes * Frame::SIZE as usize
    }

    fn retype_entry(&self) -> Result<&'static RetypeEntry, OutOfBounds> {
        let index = (self.addr().as_u64() / Frame::SIZE) as usize;
        RETYPE_TABLE
            .get()
            .unwrap()
            .retype_map
            .get(index)
            .ok_or(OutOfBounds)
    }

    fn try_as_user(self) -> Result<UserFrame, AsTypeError> {
        log::trace!("Turning {self:?} as user frame");
        self.retype_entry()?
            .get_as_and_increment(State::User)
            .map_err(|(state, value)| {
                if !matches!(state, State::User) {
                    AsTypeError::NotExpectedState(state)
                } else {
                    debug_assert!(value == RetypeEntry::MAX_REF_COUNT);
                    AsTypeError::MaxRefs(MaxRefs)
                }
            })?;
        Ok(UserFrame(self))
    }

    /// Unsafely turn a raw frame into a user frame.
    ///
    /// # Safety
    ///
    /// The raw frame must be typed as user
    unsafe fn as_user_unchecked(self) -> UserFrame {
        let frame = UserFrame(self);
        frame.entry().increment().unwrap();
        frame
    }

    fn try_as_unused_kernel(self) -> Result<KernelFrame, AsUnusedKernelError> {
        log::trace!("Trying {self:?} as an unused kernel frame");
        self.retype_entry()?
            .retype(State::Kernel, State::Kernel, 0, 1)
            .map_err(|(state, value)| {
                if !matches!(state, State::Kernel) {
                    AsUnusedKernelError::NotExpectedState(state)
                } else {
                    debug_assert!(value != 0);
                    AsUnusedKernelError::AlreadyInUse
                }
            })?;
        Ok(KernelFrame(self))
    }

    fn try_as_kernel(self) -> Result<KernelFrame, AsTypeError> {
        log::trace!("Trying {self:?} as kernel frame");
        self.retype_entry()?
            .get_as_and_increment(State::Kernel)
            .map_err(|(state, value)| {
                if !matches!(state, State::Kernel) {
                    AsTypeError::NotExpectedState(state)
                } else {
                    debug_assert!(value == RetypeEntry::MAX_REF_COUNT);
                    AsTypeError::MaxRefs(MaxRefs)
                }
            })?;
        Ok(KernelFrame(self))
    }

    fn try_as_untyped(self) -> Result<Frame, AsTypeError> {
        log::trace!("Trying to get {self:?} as untyped");
        let (state, _count) = self.retype_entry()?.get();
        if !matches!(state, State::Untyped) {
            return Err(AsTypeError::NotExpectedState(state));
        }
        Ok(self)
    }

    /// Unsafely turn a raw frame into a kernel frame.
    ///
    /// # Safety
    ///
    /// The raw frame must be typed as kernel
    unsafe fn as_kernel_unchecked(self) -> KernelFrame {
        let frame = KernelFrame(self);
        frame.entry().increment().unwrap();
        frame
    }

    fn try_into_user(self) -> Result<UserFrame, RetypeError> {
        self.retype_entry()?
            .retype(State::Untyped, State::User, 0, 1)
            .map_err(|(state, _count)| RetypeError::InvalidFromState(state))?;
        Ok(UserFrame(self))
    }

    fn try_into_kernel(self) -> Result<KernelFrame, RetypeError> {
        self.retype_entry()?
            .retype(State::Untyped, State::Kernel, 0, 1)
            .map_err(|(state, _count)| RetypeError::InvalidFromState(state))?;
        Ok(KernelFrame(self))
    }

    fn try_into_untyped_from(self, from: State) -> Result<Frame, RetypeError> {
        assert!(matches!(from, State::User | State::Kernel));
        let entry = self.retype_entry()?;

        match entry.retype(from, State::Untyped, 0, 0) {
            Ok(()) => Ok(self),
            Err((State::Unavailable, refs)) => {
                debug_assert_eq!(refs, 0);
                Err(RetypeError::InvalidFromState(State::Unavailable))
            }
            Err((s, refs)) if s == from => {
                debug_assert_ne!(refs, 0);
                Err(RetypeError::RefsExist(refs))
            }
            Err((other_state, _refs)) => Err(RetypeError::InvalidFromState(other_state)),
        }
    }

    fn try_into_untyped(self) -> Result<Frame, RetypeError> {
        if self.try_into_untyped_from(State::User).is_ok() {
            return Ok(self);
        }
        self.try_into_untyped_from(State::Kernel)?;
        Ok(self)
    }
}

impl UserFrame {
    fn entry(&self) -> &'static RetypeEntry {
        // SAFETY: Entry must exist if a KernelFrame exists.
        unsafe { self.0.retype_entry().unwrap_unchecked() }
    }

    pub fn raw(&self) -> Frame {
        self.0
    }

    /// Builds back a user frame from the raw frame
    ///
    /// # Safety
    ///
    /// The frame must have been created with `into_raw`.
    pub unsafe fn from_raw(frame: Frame) -> Self {
        Self(frame)
    }

    pub fn into_raw(self) -> Frame {
        ManuallyDrop::new(self).0
    }

    pub fn try_clone(&self) -> Option<Self> {
        self.0.retype_entry().unwrap().increment().ok()?;
        Some(Self(self.raw()))
    }

    pub fn drop(self) -> u16 {
        let this = ManuallyDrop::new(self);
        let count = this.entry().decrement().unwrap();
        log::debug!("Dropping {this:?}: Old count {count}");
        count
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct KernelFrame(Frame);

impl KernelFrame {
    fn entry(&self) -> &'static RetypeEntry {
        // SAFETY: Entry must exist if a KernelFrame exists.
        unsafe { self.0.retype_entry().unwrap_unchecked() }
    }

    pub fn raw(&self) -> Frame {
        self.0
    }

    pub fn into_raw(self) -> Frame {
        ManuallyDrop::new(self).0
    }

    /// Builds back a kernel frame from the raw frame
    ///
    /// # Safety
    ///
    /// The frame must have been created with `into_raw`.
    pub unsafe fn from_raw(frame: Frame) -> Self {
        Self(frame)
    }

    pub fn try_clone(&self) -> Option<Self> {
        self.0.retype_entry().unwrap().increment().ok()?;
        Some(Self(self.raw()))
    }

    pub fn drop(self) -> u16 {
        let this = ManuallyDrop::new(self);
        let count = this.entry().decrement().unwrap();
        log::debug!("Dropping {this:?}: Old count {count}");
        count
    }
}

impl Drop for KernelFrame {
    fn drop(&mut self) {
        let count = self.entry().decrement().unwrap();
        log::debug!("Dropping {self:?}: Old count {count}");
    }
}

impl Drop for UserFrame {
    fn drop(&mut self) {
        let count = self.entry().decrement().unwrap();
        log::debug!("Dropping {self:?}: Old count {count}");
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct RetypeEntry(AtomicU16);

#[derive(Debug)]
struct Invalid;
impl State {
    const fn try_from(value: u8) -> Result<Self, Invalid> {
        match value {
            0 => Ok(State::Unavailable),
            1 => Ok(State::Untyped),
            2 => Ok(State::User),
            3 => Ok(State::Kernel),
            _ => Err(Invalid),
        }
    }
}

#[allow(unused)]
impl RetypeEntry {
    const STATE_BITS: u16 = 2;
    const COUNTER_BITS: u16 = 16 - Self::STATE_BITS;
    pub const MAX_REF_COUNT: u16 = (1 << Self::COUNTER_BITS) - 1;

    fn value_for(state: State, counter: u16) -> u16 {
        assert!(counter <= Self::MAX_REF_COUNT);

        ((state as u8 as u16) << Self::COUNTER_BITS) + counter % Self::MAX_REF_COUNT
    }

    const fn value_into(value: u16) -> (State, u16) {
        let counter = value & ((1 << Self::COUNTER_BITS) - 1);
        let state = match State::try_from((value >> Self::COUNTER_BITS) as u8) {
            Ok(state) => state,
            Err(_e) => panic!("Invalid retype state"),
        };
        (state, counter)
    }

    pub fn unavailable() -> Self {
        Self(AtomicU16::new(Self::value_for(State::Unavailable, 0)))
    }

    pub fn untyped() -> Self {
        Self(AtomicU16::new(Self::value_for(State::Untyped, 0)))
    }

    pub fn kernel(ref_count: u16) -> Self {
        Self(AtomicU16::new(Self::value_for(State::Kernel, ref_count)))
    }

    pub fn increment(&self) -> Result<u16, MaxRefs> {
        self.0
            .fetch_update(Ordering::Release, Ordering::Relaxed, |value| {
                let (_, counter) = Self::value_into(value);
                if counter == Self::MAX_REF_COUNT {
                    None
                } else {
                    Some(value + 1)
                }
            })
            .map(|entry| Self::value_into(entry).1)
            .map_err(|_| MaxRefs)
    }

    pub fn decrement(&self) -> Result<u16, NoRefs> {
        self.0
            .fetch_update(Ordering::Release, Ordering::Relaxed, |value| {
                let (_, counter) = Self::value_into(value);
                if counter == 0 {
                    None
                } else {
                    Some(value - 1)
                }
            })
            .map(|entry| Self::value_into(entry).1)
            .map_err(|_| NoRefs)
    }

    pub fn get(&self) -> (State, u16) {
        Self::value_into(self.0.load(Ordering::Relaxed))
    }

    pub fn get_as_and_increment(&self, wants: State) -> Result<(), (State, u16)> {
        self.0
            .fetch_update(Ordering::Release, Ordering::Relaxed, |value| {
                let (state, count) = Self::value_into(value);
                if wants == state && count < Self::MAX_REF_COUNT {
                    Some(Self::value_for(state, count + 1))
                } else {
                    None
                }
            })
            .map(|_| ())
            .map_err(Self::value_into)
    }

    pub fn retype(
        &self,
        from_state: State,
        to_state: State,
        from_counter: u16,
        to_counter: u16,
    ) -> Result<(), (State, u16)> {
        let from = Self::value_for(from_state, from_counter);
        let to = Self::value_for(to_state, to_counter);
        self.0
            .compare_exchange(from, to, Ordering::Relaxed, Ordering::Relaxed)
            .map_err(Self::value_into)?;

        Ok(())
    }

    pub fn set(&mut self, state: State, value: u16) {
        let to = Self::value_for(state, value);
        *self.0.get_mut() = to;
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[repr(u8)]
pub enum State {
    Unavailable = 0,
    Untyped = 1,
    User = 2,
    Kernel = 3,
}
