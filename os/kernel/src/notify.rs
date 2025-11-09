use derive_where::derive_where;
use qapi::caps::CapError;

use crate::arch::{ArchSystem, System};
use crate::kmem::KPtr;
use crate::thread::{DispatchToken, Thread};

#[derive_where(Debug, Clone)]
pub struct Notification<S: System> {
    waiter: KPtr<Thread<S>>,
    badge: u32,
}

impl<S: System> Notification<S> {
    pub const fn new(waiter: KPtr<Thread<S>>, badge: u32) -> Self {
        Self { waiter, badge }
    }

    pub const fn waiter(&self) -> &KPtr<Thread<S>> {
        &self.waiter
    }
}

impl Notification<ArchSystem> {
    pub fn signal(
        &self,
        ctx: <ArchSystem as System>::IrqCtx,
    ) -> Result<Option<DispatchToken<ArchSystem>>, CapError> {
        Thread::notify(&self.waiter, self.badge, ctx)
    }
}
