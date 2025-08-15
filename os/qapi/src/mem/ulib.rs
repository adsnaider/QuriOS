use crate::{
    caps::CapError,
    syscall::{SyscallArgs, SyscallOp, ulib::syscall},
};

use super::{
    Frame,
    phys::{RetypeKind, RetypeOp},
};

impl Frame {
    pub fn retype(&self, to: RetypeKind) -> Result<(), CapError> {
        let op = SyscallOp::Retype;
        let args = RetypeOp {
            frame: self.base(),
            to,
        }
        .into_args();
        syscall(SyscallArgs::new_with_args(op, args)).map(|_| ())
    }
}
