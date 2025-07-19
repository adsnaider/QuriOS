use derive_more::{Display, Error, Into, TryFrom};
use trie::TrieIndexError;
use zerocopy::{FromBytes, Immutable, IntoBytes};

pub mod cap_table;
pub mod page_table;

#[cfg(target_arch = "x86_64")]
pub const PAGE_SIZE: usize = 4096;
pub const SLOT_SIZE: usize = 128;

pub const NUM_SLOTS: usize = PAGE_SIZE / SLOT_SIZE;

pub type SlotId = trie::SlotId<NUM_SLOTS>;

#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum CapabilityKind {
    Empty = 0,
    Thread,
    TranscientPageTable,
    RootPageTable,
    CapTable,
    SyncCall,
    Retype,
}

#[derive(
    Debug,
    Display,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    IntoBytes,
    FromBytes,
    Immutable,
)]
#[repr(transparent)]
pub struct CapId(u32);

impl TryFrom<usize> for CapId {
    type Error = CapError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(Self(
            value.try_into().map_err(|_| CapError::CapIndexOutOfRange)?,
        ))
    }
}

impl From<CapId> for usize {
    fn from(value: CapId) -> Self {
        value.0.try_into().unwrap()
    }
}

impl CapId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn value(&self) -> u32 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, Display, Error, TryFrom)]
#[try_from(repr)]
#[repr(isize)]
pub enum CapError {
    #[display("Unknown capability error, possibly due to an invalid syscall response")]
    Unknown = -1,
    #[display("Capability index is out of range of maximum allowed")]
    CapIndexOutOfRange = -2,
    #[display("Capability index does not point to an active capability")]
    CapNotFound = -3,
    #[display("Invalid syscall argument wasn't typed properly")]
    InvalidArg = -4,
    #[display("Invalid syscall operation isn't valid for capability")]
    InvalidOp = -5,
    #[display("Invalid user pointer triggered a page fault while reading or writing")]
    BadUserMemory = -6,
}

impl CapError {
    pub fn from_isize(value: isize) -> Self {
        Self::try_from(value).unwrap_or(CapError::Unknown)
    }
}

impl From<CapError> for isize {
    fn from(value: CapError) -> Self {
        value as isize
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Into)]
#[repr(transparent)]
pub struct PositiveIsize(isize);

impl TryFrom<isize> for PositiveIsize {
    type Error = PositiveIsizeUnderflow;

    fn try_from(value: isize) -> Result<Self, Self::Error> {
        if value >= 0 {
            Ok(Self(value))
        } else {
            Err(PositiveIsizeUnderflow)
        }
    }
}

#[derive(Debug, Copy, Clone, Display, Error)]
#[display("The isize is not positive")]
pub struct PositiveIsizeUnderflow;

pub trait CapResult {
    fn into_isize(self) -> isize;
    fn from_isize(value: isize) -> Self;
}

impl CapResult for Result<PositiveIsize, CapError> {
    fn into_isize(self) -> isize {
        match self {
            Ok(ok) => ok.into(),
            Err(e) => isize::from(e),
        }
    }

    fn from_isize(value: isize) -> Self {
        match value.try_into() {
            Ok(pos) => Ok(pos),
            Err(_) => Err(CapError::from_isize(value)),
        }
    }
}

impl From<TrieIndexError> for CapError {
    fn from(_: TrieIndexError) -> Self {
        Self::CapIndexOutOfRange
    }
}
