use derive_more::{Display, Error, Into, TryFrom};

#[derive(Debug)]
#[repr(u8)]
pub enum CapabilityKind {
    Empty = 0,
    Thread,
    TranscientPageTable,
    RootPageTable,
    CapTable,
}

#[derive(Debug, Display)]
pub struct CapIndex(u32);

impl TryFrom<usize> for CapIndex {
    type Error = CapError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(Self(
            value.try_into().map_err(|_| CapError::CapIndexOutOfRange)?,
        ))
    }
}

impl CapIndex {
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
    #[display("Capability index points to an empty capability slot")]
    CapSlotIsEmpty = -3,
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
