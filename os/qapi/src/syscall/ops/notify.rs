use qapi_macros::SyscallRequest;

use crate::caps::notify::NotificationCap;

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct NotifyOp {
    pub notify_cap: NotificationCap,
}
