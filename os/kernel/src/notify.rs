use derive_where::derive_where;
use qapi::caps::CapError;

use crate::{
    arch::{ArchSystem, System},
    kmem::KPtr,
    thread::Thread,
};

#[derive_where(Debug, Clone)]
pub struct Notification<S: System> {
    waiter: KPtr<Thread<S>>,
}

impl<S: System> Notification<S> {
    pub const fn new(waiter: KPtr<Thread<S>>) -> Self {
        Self { waiter }
    }

    pub const fn waiter(&self) -> &KPtr<Thread<S>> {
        &self.waiter
    }
}

impl Notification<ArchSystem> {
    pub fn signal(&self, ctx: <ArchSystem as System>::IrqCtx) -> Result<(), CapError> {
        let dispatcher = Thread::priority_dispatch(self.waiter.clone(), ctx)?;
        if let Some(dispatcher) = dispatcher {
            dispatcher.dispatch();
        } else {
            Ok(())
        }
    }
}
