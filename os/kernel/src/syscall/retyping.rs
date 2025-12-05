use qapi::caps::{CapError, PositiveIsize};
use qapi::syscall::ops::retype::{RetypeKind, RetypeOp};

use crate::arch::mem::Frame;
use crate::retyping::{FrameExt, OutOfBounds, RetypeError, State};

pub fn retype(RetypeOp { frame, to }: RetypeOp) -> Result<PositiveIsize, CapError> {
    // TODO: Verify that frame is in component's capability set
    let frame = Frame::try_from(frame)?;
    match to {
        RetypeKind::IntoUntyped => frame.try_to_untyped()?,
        RetypeKind::IntoKernel => frame.try_to_kernel()?,
        RetypeKind::IntoUser => frame.try_to_user()?,
    }
    Ok(PositiveIsize::zero())
}

impl From<RetypeError> for CapError {
    fn from(value: RetypeError) -> Self {
        match value {
            RetypeError::InvalidFromState(State::Unavailable) => CapError::FrameUnavailable,
            RetypeError::InvalidFromState(State::Untyped) => CapError::FrameAlreadyUntyped,
            RetypeError::InvalidFromState(State::Kernel | State::User) => {
                CapError::FrameAlreadyTyped
            }
            RetypeError::RefsExist(_) => CapError::FrameRefsExist,
            RetypeError::OutOfBounds(OutOfBounds) => CapError::FrameOffAvailableMemoryRange,
        }
    }
}
