use core::cell::{Ref, RefCell};
use core::mem::MaybeUninit;

use derive_more::{Display, Error};
use derive_where::derive_where;
use heapless::Vec;
use qapi::caps::{CapError, CapId};
use qapi::syscall::ops::introspect;
use qapi::syscall::ops::sync_ipc::{SyncRetOp, SYNC_CALL_ARGS};

use crate::arch::mem::Addrspace;
use crate::arch::{ArchSystem, System};
use crate::arch::{ExecState, InvokeAbi};
use crate::caps::{CapRef, CapTable, Resources};
use crate::core_local::core_cell::{Affinity, BorrowError, ResetAffinityError, SetAffinityError};
use crate::core_local::{CoreCell, CORE_LOCAL_CORE_ID, CORE_LOCAL_CURRENT_THREAD};
use crate::kmem::KPtr;
use crate::never::Never;
use crate::sync_call::{CallAbi, SyncCall};

pub type CurrentThread = RefCell<Option<KPtr<Thread<ArchSystem>>>>;

#[derive_where(Debug, Clone)]
struct ThreadCtx<S: System> {
    exec_state: S::ExecState,
    resources: KPtr<Resources<S>>,
}

#[derive_where(Debug)]
pub struct Thread<S: System> {
    ctx: CoreCell<Vec<ThreadCtx<S>, 16>>,
}

impl Thread<ArchSystem> {
    fn replace_current(new: KPtr<Self>) -> Option<KPtr<Self>> {
        CORE_LOCAL_CURRENT_THREAD.replace(Some(new))
    }

    pub fn with_current<F, T>(fun: F) -> T
    where
        F: FnOnce(&KPtr<Self>) -> T,
    {
        let current = CORE_LOCAL_CURRENT_THREAD.borrow();
        let current = current.as_ref().unwrap();
        #[cfg(debug_assertions)]
        current
            .verify_affinity()
            .expect("Current thread is not bound to this core");
        // SAFETY: repr transparent and identical semantics over
        fun(current)
    }

    pub fn initial_dispatch(this: KPtr<Self>) -> Result<Never, SetAffinityError> {
        let exec_state = {
            match this.ctx.set_affinity() {
                Ok(()) => {}
                Err(SetAffinityError::BoundToSelf) => {}
                Err(e) => return Err(e),
            }
            this.active_comp().unwrap().addrspace().activate();
            // TODO: Remove lint allow once type alias impl trait works and ArchSystem uses it.
            #[allow(clippy::clone_on_copy)]
            let exec_state = this.active_ctx().unwrap().exec_state.clone();
            assert!(Self::replace_current(this).is_none());
            log::info!("Set the active thread");
            exec_state
        };
        exec_state.dispatch();
    }
    pub fn dispatch(
        this: KPtr<Self>,
        ctx: <ArchSystem as System>::IrqCtx,
    ) -> Result<Never, SetAffinityError> {
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
        let exec_state = {
            match this.ctx.set_affinity() {
                Ok(()) => {}
                Err(SetAffinityError::BoundToSelf) => {}
                Err(e) => return Err(e),
            }
            this.active_comp().unwrap().addrspace().activate();
            // TODO: Remove lint allow once type alias impl trait works and ArchSystem uses it.
            #[allow(clippy::clone_on_copy)]
            let exec_state = this.active_ctx().unwrap().exec_state.clone();
            {
                let previous = Self::replace_current(this);
                if let Some(previous) = &previous {
                    previous.active_ctx().unwrap().exec_state.save(&ctx);
                    previous.unset_affinity().expect(
                        "Previous thread did not have it's affinity properly set to the local core",
                    );
                    // At this point, any other core may come in and execute the previous thread which is fine as it was saved above and it won't be used further
                }
            }
            log::info!("Set the active thread");
            exec_state
        };
        exec_state.dispatch();
    }
}

impl<S: System> Thread<S> {
    pub fn new(exec_state: S::ExecState, comp: KPtr<Resources<S>>) -> Self {
        let mut ctx = Vec::new();
        ctx.push(ThreadCtx {
            exec_state,
            resources: comp,
        })
        .unwrap();
        Self {
            ctx: CoreCell::new(ctx),
        }
    }

    pub fn get_cap(&self, cap: CapId) -> Result<Option<CapRef<S>>, BorrowError> {
        Ok(CapTable::get(self.active_comp()?.cap_table(), cap.value()))
    }

    pub fn active_comp(&self) -> Result<Ref<'_, KPtr<Resources<S>>>, BorrowError> {
        Ok(Ref::map(self.active_ctx()?, |ctx| &ctx.resources))
    }

    pub fn introspect(&self) -> introspect::Thread {
        introspect::Thread
    }

    fn active_ctx(&self) -> Result<Ref<'_, ThreadCtx<S>>, BorrowError> {
        Ok(Ref::map(self.ctx.try_borrow()?, |ctx| ctx.last().unwrap()))
    }

    pub fn sync_invoke<Abi>(
        &self,
        sync_call: SyncCall<S, Abi>,
        ctx: S::IrqCtx,
    ) -> Result<Never, CapError>
    where
        Abi: InvokeAbi<S>,
    {
        let exec_state = {
            let (resources, xstate) = sync_call.create_invocation(&ctx);
            let sync_ctx = ThreadCtx {
                resources,
                exec_state: xstate,
            };

            let mut curr_ctx = self
                .ctx
                .try_borrow_mut()
                .expect("Attempted to synchronous invoke on non-bound thread");
            curr_ctx
                .last()
                .as_mut()
                .expect("There should always be at least 1 execution context on a thread")
                .exec_state
                .save(&ctx);
            curr_ctx
                .push(sync_ctx.clone())
                .map_err(|_| CapError::SyncInvokeLimit)?;
            sync_ctx.resources.addrspace().activate();
            sync_ctx.exec_state.clone()
        };
        exec_state.dispatch();
    }

    pub fn sync_ret(&self, args: SyncRetOp) -> Result<Never, CapError> {
        let exec_state = {
            let mut ctx = self
                .ctx
                .try_borrow_mut()
                .expect("Kernel attempted to sync ret on non core bound thread");
            if ctx.len() <= 1 {
                return Err(CapError::SyncRetLimit);
            }
            let _prev_ctx = ctx.pop().unwrap();
            let ret_ctx = ctx.last().unwrap();
            ret_ctx.resources.addrspace().activate();
            ret_ctx.exec_state.update_sync_ret(args.resp);
            ret_ctx.exec_state.clone()
        };
        exec_state.dispatch();
    }

    pub fn set_affinity(&self) -> Result<(), SetAffinityError> {
        self.ctx.set_affinity()
    }

    pub fn unset_affinity(&self) -> Result<(), ResetAffinityError> {
        self.ctx.reset_affinity()
    }

    fn verify_affinity(&self) -> Result<(), FugitiveThread> {
        if self.ctx.is_bound_to_local() {
            Ok(())
        } else {
            Err(FugitiveThread)
        }
    }
}

#[derive(Debug, Display, Error)]
#[display("Found a fugitive thread not boiund to the expected core")]
struct FugitiveThread;
