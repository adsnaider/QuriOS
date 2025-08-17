use core::cell::RefCell;

use qapi::caps::CapId;

use crate::arch::exec::ExecState;
use crate::arch::mem::Addrspace;
use crate::arch::{ArchSystem, System};
use crate::caps::{CapRef, CapTable, Resources};
use crate::core_local::CORE_LOCAL_CURRENT_THREAD;
use crate::kmem::KPtr;

pub type CurrentThread = RefCell<Option<KPtr<Thread<ArchSystem>>>>;

#[repr(C)]
#[derive(Debug)]
pub struct Thread<S: System> {
    exec_state: S::ExecState,
    resources: Resources<S>,
}

impl Thread<ArchSystem> {
    fn replace_current(new: KPtr<Self>) -> Option<KPtr<Self>> {
        CORE_LOCAL_CURRENT_THREAD.replace(Some(new))
    }

    pub fn get_cap(cap: CapId) -> Option<CapRef<ArchSystem>> {
        CapTable::get(
            CORE_LOCAL_CURRENT_THREAD
                .borrow()
                .as_ref()
                .unwrap()
                .active_comp()
                .cap_table(),
            cap.value(),
        )
    }
    pub fn dispatch_diverging(to: KPtr<Self>) -> ! {
        to.active_comp().addrspace().activate();
        // TODO: Remove lint allow once type alias impl trait works and ArchSystem uses it.
        #[allow(clippy::clone_on_copy)]
        let exec_state = to.exec_state.clone();
        assert!(Self::replace_current(to).is_none());
        log::info!("Set the active thread");
        exec_state.dispatch();
    }

    pub fn dispatch(
        this: KPtr<Self>,
        ctx: &<<ArchSystem as System>::ExecState as ExecState>::RegCtx,
    ) -> ! {
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
            let previous = Self::replace_current(this);
            if let Some(previous) = &previous {
                previous.exec_state.save(ctx);
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
