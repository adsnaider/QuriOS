use arch::{exec::ExecState, mem::Addrspace, ArchSystem, System};

use crate::{caps::Resources, kmem::KPtr};

#[repr(C)]
#[derive(Debug)]
pub struct Thread<S: System> {
    exec_state: S::ExecState,
    resources: Resources<S>,
}

impl Thread<ArchSystem> {
    fn replace_current(new: KPtr<Self>, current: &mut Option<KPtr<Self>>) -> Option<KPtr<Self>> {
        current.replace(new)
    }

    pub fn dispatch(this: KPtr<Self>, current: &mut Option<KPtr<Self>>) -> ! {
        // Our kernel is non-preemptive which makes every other case really
        // simple as it's a completely synchronous call-response. However, thread
        // dispatching is somewhat weird because we exit the kernel early on the
        // dispatch and never return back to the caller in a traditional sense (i.e.
        // dispatch returns !). The way we come back is by having another dispatch
        // call back into the original thread. Note, we have a singular kernel
        // execution stack, so once we leave here, the stack will be mangled and
        // can't come back to the kernel to return to the normal flow of execution.
        //
        // When that happens, the state of the (current) thread needs to be valid,
        // specifically, to the thread it needs to look like the original Activate
        // call returned with a success status code. So here's what needs to happen
        //
        // 1. Return register needs to be 0.
        // 2. rflags register needs to be valid (interrupts enabled, ring 3 execution, etc.)
        // 3. stack register needs to be whatever it was before syscall
        // 4. All callee-saved registers need to be set back (done in userspace)
        this.active_comp().addrspace().activate();
        // TODO: Remove lint allow once type alias impl trait works and ArchSystem uses it.
        #[allow(clippy::clone_on_copy)]
        let exec_state = this.exec_state.clone();
        {
            let previous = Self::replace_current(this, current);
            if let Some(previous) = &previous {
                previous.exec_state.save();
            }
        }
        log::info!("Set the active thread");
        exec_state.dispatch();
    }
}

impl<S: System> Thread<S> {
    pub fn new(exec_state: S::ExecState, comp: Resources<S>) -> Self {
        Self {
            exec_state,
            resources: comp,
        }
    }

    pub fn active_comp(&self) -> &Resources<S> {
        &self.resources
    }
}
