use crate::{
    caps::CapError,
    syscall::{SyscallArgs, SyscallOp, ulib::syscall},
};

use super::{DispatchOp, Thread};

impl Thread {
    pub fn dispatch(&self) -> Result<(), CapError> {
        let args = DispatchOp {
            thread_cap: self.cap,
        }
        .into_args();
        syscall(SyscallArgs::new_with_args(SyscallOp::ThreadDispatch, args)).map(|_| ())
    }
}
