use qapi_macros::SyscallRequest;

use crate::caps::irq_ctrl::IrqCtrlCap;
use crate::caps::notify::NotificationCap;

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct IrqSet {
    pub irq_ctrl: IrqCtrlCap,
    pub notification: NotificationCap,
    pub irq: usize,
}

#[derive(Debug, Copy, Clone, SyscallRequest)]
pub struct IrqUnset {
    pub irq_ctrl: IrqCtrlCap,
    pub irq: usize,
}
