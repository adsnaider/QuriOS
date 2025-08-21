use core::convert::Infallible;

use derive_more::{Display, Error, Into, TryFrom, TryFromReprError};
use zerocopy::{FromBytes, Immutable, IntoBytes};

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
    #[display("The slot ID would overflow the resource table")]
    InvalidTableSlot = -7,
    #[display("The requested syscall operation is currently unimplemented")]
    SyscallNotImplemented = -8,
    #[display("The frame provided is not typed as kernel")]
    NotKernelTyped = -9,
    #[display("The frame provided is already in use")]
    FrameInUse = -10,
    #[display("The capability is not empty (and cannot be set)")]
    CapNotEmpty = -11,
    #[display("The frame provided is permanently unavailable")]
    FrameUnavailable = -12,
    #[display("The frame is already untyped")]
    FrameAlreadyUntyped = -13,
    #[display("The frame is typed and must be untyped first")]
    FrameAlreadyTyped = -14,
    #[display("Can't untype due to existing references")]
    FrameRefsExist = -15,
    #[display("Frame exceeds the system's physical memory range")]
    FrameOffAvailableMemoryRange = -16,
    #[display(
        "Virtual memory tables can only be linked in a linear fashion to prevent recursive mappings"
    )]
    VMLinkNotFlat = -17,
    #[display("Some capability in the arguments doesn't match the expected type for the operation")]
    InvalidCapType = -18,
    #[display("Reached the static sysnchronous invocation stack limit")]
    SyncInvokeLimit = -19,
    #[display(
        "Failure to return from synchronous invocation since this is the bottom of the sync call stack"
    )]
    SyncRetLimit = -20,
    #[display("The VM Table level provided is not valid for this architecture")]
    InvalidVMTableLevel = -21,
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Into, Display)]
#[repr(transparent)]
pub struct PositiveIsize(isize);

impl PositiveIsize {
    pub const fn zero() -> Self {
        Self(0)
    }
}

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

impl From<PositiveIsize> for usize {
    fn from(value: PositiveIsize) -> Self {
        value.0 as usize
    }
}

impl TryFrom<usize> for PositiveIsize {
    type Error = CapError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        let value = isize::try_from(value).map_err(|_| CapError::InvalidArg)?;
        Ok(Self(value))
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

impl From<TryFromReprError<usize>> for CapError {
    fn from(_: TryFromReprError<usize>) -> Self {
        Self::InvalidArg
    }
}

impl From<Infallible> for CapError {
    fn from(_: Infallible) -> Self {
        unreachable!();
    }
}
