use derive_more::{Display, Error, From, Into};
use tap::Tap as _;

#[derive(Debug)]
pub enum CapabilityKind {
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

#[derive(Debug, Copy, Clone, From, Display, Error)]
#[repr(isize)]
pub enum CapError {
    #[display("Capability index is out of range of maximum allowed")]
    CapIndexOutOfRange = -1,
}

impl From<CapError> for isize {
    fn from(value: CapError) -> Self {
        (value as isize).tap(|v| debug_assert!(*v < 0))
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
}

impl CapResult for Result<PositiveIsize, CapError> {
    fn into_isize(self) -> isize {
        match self {
            Ok(ok) => ok.into(),
            Err(e) => isize::from(e),
        }
    }
}
