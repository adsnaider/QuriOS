use qapi_macros::SyscallRequest;

use crate::caps::thread::ThreadCap;

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct DispatchOp {
    pub thread_cap: ThreadCap,
}
