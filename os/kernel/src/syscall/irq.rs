use qapi::caps::CapError;
use qapi::syscall::ops::irq::{IrqSet, IrqUnset};

use super::SyscallResp;
use crate::arch::ArchCaps;
use crate::thread::Thread;

pub fn irq_set(
    IrqSet {
        irq_ctrl,
        notification,
        irq,
    }: IrqSet,
) -> SyscallResp {
    Thread::with_current(|thread| {
        let irq_ctrl = thread
            .get_cap(irq_ctrl.cap())
            .ok_or(CapError::CapNotFound)?
            .as_arch_cap()?
            .clone();
        let notification = thread
            .get_cap(notification.cap())
            .ok_or(CapError::CapNotFound)?
            .as_notification()?
            .clone();

        irq_ctrl.irq_set(irq, Some(notification))
    })
}

pub fn irq_unset(IrqUnset { irq_ctrl, irq }: IrqUnset) -> SyscallResp {
    Thread::with_current(|thread| {
        let irq_ctrl = thread
            .get_cap(irq_ctrl.cap())
            .ok_or(CapError::CapNotFound)?
            .as_arch_cap()?
            .clone();

        irq_ctrl.irq_set(irq, None)
    })
}
