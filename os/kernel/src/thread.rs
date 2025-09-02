use core::ops::Deref;

use derive_more::{Display, Error};
use derive_where::derive_where;
use heapless::Vec;
use qapi::caps::sync_ipc::{ExceptionAbi, StandardAbi};
use qapi::caps::{CapError, CapId};
use qapi::syscall::ops::introspect;
use qapi::syscall::ops::sync_ipc::SyncRetOp;

use crate::arch::mem::Addrspace;
use crate::arch::{ArchSystem, ExecState, InvokeAbi, System};
use crate::caps::{CapRef, CapTable, Resources};
use crate::core_local::core_cell::{BindError, CoreGuard, Lock, UnbindError};
use crate::core_local::{CORE_LOCAL_CURRENT_THREAD, CoreCell};
use crate::kmem::KPtr;
use crate::never::Never;
use crate::sync_call::AnyAbi;

pub type CoreLocalThread = Lock<Option<KPtr<Thread<ArchSystem>>>>;

#[derive_where(Debug)]
pub struct ThreadExecStack<S: System>(Vec<ThreadExecCtx<S>, 16>);

impl<S: System> ThreadExecStack<S> {
    pub fn active(&self) -> &ThreadExecCtx<S> {
        self.0.last().unwrap()
    }

    pub fn active_mut(&mut self) -> &mut ThreadExecCtx<S> {
        self.0.last_mut().unwrap()
    }

    pub fn is_full(&self) -> bool {
        self.0.is_full()
    }

    pub fn is_base(&self) -> bool {
        self.0.len() == 1
    }

    pub fn push(&mut self, ctx: ThreadExecCtx<S>) -> Result<(), ThreadExecCtx<S>> {
        self.0.push(ctx)
    }

    pub fn pop(&mut self) -> Option<ThreadExecCtx<S>> {
        if self.is_base() {
            None
        } else {
            Some(self.0.pop().unwrap())
        }
    }
}

#[derive_where(Debug)]
pub struct ThreadExecCtx<S: System> {
    exec_state: S::ExecState,
    resources: KPtr<Resources<S>>,
    abi: AnyAbi,
}

impl<S: System> ThreadExecCtx<S> {
    pub fn new(state: S::ExecState, resources: KPtr<Resources<S>>, abi: impl Into<AnyAbi>) -> Self {
        Self {
            exec_state: state,
            resources,
            abi: abi.into(),
        }
    }

    pub fn get_cap(&self, cap: CapId) -> Option<CapRef<S>> {
        CapTable::get(self.resources.ctable(), cap.value())
    }

    pub fn addrspace(&self) -> impl Deref<Target = S::Addrspace> + '_ {
        self.resources.addrspace()
    }

    pub fn ctable(&self) -> &KPtr<CapTable<S>> {
        self.resources.ctable()
    }
}

#[derive_where(Debug)]
pub struct Thread<S: System> {
    execution_stack: CoreCell<Lock<ThreadExecStack<S>>>,
    flat_priority: u32,
}

#[derive_where(Debug)]
#[repr(transparent)]
pub struct LocalBoundThread<S: System>(Thread<S>);

impl Thread<ArchSystem> {
    pub fn current() -> &'static CoreLocalThread {
        &CORE_LOCAL_CURRENT_THREAD
    }

    fn replace_current(new: KPtr<Self>) -> Option<KPtr<Self>> {
        CORE_LOCAL_CURRENT_THREAD.replace(Some(new)).unwrap()
    }

    pub fn with_current<F, T>(fun: F) -> T
    where
        F: FnOnce(&LocalBoundThread<ArchSystem>) -> T,
    {
        Self::current().locked(|current| {
            let current = current.as_mut().unwrap();
            let current = current.verify_affinity().unwrap();
            // SAFETY: repr transparent and identical semantics over
            fun(current)
        })
    }

    pub fn kinit_dispatch(this: KPtr<Self>) -> Result<Never, BindError> {
        this.execution_stack.bind()?;
        let exec_state = this
            .execution_stack
            .with(|ctx| {
                let ctx = ctx.lock();
                ctx.active().addrspace().activate();
                // TODO: Remove lint allow once type alias impl trait works and ArchSystem uses it.
                #[allow(clippy::clone_on_copy)]
                ctx.active().exec_state.clone()
            })
            .expect("As bound above");
        assert!(Self::replace_current(this).is_none());
        log::info!("Set the active thread");
        exec_state.dispatch();
    }

    pub fn dispatch(
        this: KPtr<Self>,
        irq_ctx: <ArchSystem as System>::IrqCtx,
    ) -> Result<Never, CapError> {
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

        let exec_state;
        {
            this.bind()?;
            let previous = Self::replace_current(this);
            exec_state = Self::with_current(move |next| {
                let exec_stack = next.execution_stack();
                let exec_stack = exec_stack.lock();
                exec_stack.active().addrspace().activate();
                // TODO: Remove lint allow once type alias impl trait works and ArchSystem uses it.
                #[allow(clippy::clone_on_copy)]
                let exec_state = exec_stack.active().exec_state.clone();
                if let Some(previous) = previous {
                    let previous = previous
                        .verify_affinity()
                        .expect("Previous thread was not bound to this core!");
                    let prev_ctx = previous.execution_stack();
                    let prev_ctx = prev_ctx.lock();
                    prev_ctx.active().exec_state.save(&irq_ctx);
                    previous
                        .unbind()
                        .expect("Active references to previous thread preventing unbinding!");
                    // At this point, any other core may come in and execute the previous thread which is fine as it was saved above and it won't be used further
                }
                exec_state
            });
            log::info!("Set the active thread");
        }
        exec_state.dispatch();
    }

    pub fn priority_dispatch(
        next: KPtr<Self>,
        ctx: <ArchSystem as System>::IrqCtx,
    ) -> Result<(), CapError> {
        let dispatch = Self::with_current(|current| next.flat_priority() > current.flat_priority());
        if dispatch {
            Self::dispatch(next, ctx)?;
        } else {
            Ok(())
        }
    }
}

impl<S: System> Thread<S> {
    pub fn new(exec_state: S::ExecState, comp: KPtr<Resources<S>>, priority: u32) -> Self {
        let mut ctx = Vec::new();
        ctx.push(ThreadExecCtx {
            exec_state,
            resources: comp,
            abi: Default::default(),
        })
        .unwrap();
        Self {
            execution_stack: CoreCell::new(Lock::new(ThreadExecStack(ctx))),
            flat_priority: priority,
        }
    }

    pub fn introspect(&self) -> introspect::Thread {
        introspect::Thread
    }

    pub fn bind(&self) -> Result<&LocalBoundThread<S>, BindError> {
        self.execution_stack.bind()?;
        Ok(self.verify_affinity().unwrap())
    }

    pub fn unbind(&self) -> Result<(), UnbindError> {
        self.execution_stack.unbind()
    }

    fn verify_affinity(&self) -> Result<&LocalBoundThread<S>, FugitiveThread> {
        LocalBoundThread::new(self)
    }

    pub fn flat_priority(&self) -> u32 {
        self.flat_priority
    }
}

impl<S: System> LocalBoundThread<S> {
    pub fn new(thread: &Thread<S>) -> Result<&Self, FugitiveThread> {
        if thread.execution_stack.is_locally_bound() {
            Ok(unsafe { core::mem::transmute(thread) })
        } else {
            Err(FugitiveThread)
        }
    }

    pub fn unbind(&self) -> Result<(), UnbindError> {
        self.0.unbind()
    }

    pub fn execution_stack(&self) -> CoreGuard<'_, Lock<ThreadExecStack<S>>> {
        self.0.execution_stack.try_get().unwrap()
    }

    pub fn invoke(&self, invocation: ThreadExecCtx<S>, ctx: S::IrqCtx) -> Result<Never, CapError> {
        let exec_state = {
            let curr_ctx = self.execution_stack();
            let mut curr_ctx = curr_ctx.lock();
            if curr_ctx.is_full() {
                return Err(CapError::SyncInvokeLimit);
            }
            curr_ctx.active_mut().exec_state.save(&ctx);
            invocation.resources.addrspace().activate();
            let xstate = invocation.exec_state.clone();
            curr_ctx.push(invocation).unwrap();
            xstate
        };
        exec_state.dispatch();
    }

    pub fn sync_ret(&self, _args: SyncRetOp, callee_ctx: S::IrqCtx) -> Result<Never, CapError>
    where
        StandardAbi: InvokeAbi<S>,
        ExceptionAbi: InvokeAbi<S>,
    {
        let exec_state = {
            let thread_ctx = self.execution_stack();
            let mut thread_ctx = thread_ctx.lock();
            if thread_ctx.is_base() {
                return Err(CapError::SyncRetLimit);
            }

            let ThreadExecCtx { abi, .. } = thread_ctx.pop().unwrap();
            let caller_ctx = thread_ctx.active_mut();
            abi.ret_to(&callee_ctx, &caller_ctx.exec_state)?;
            caller_ctx.resources.addrspace().activate();
            caller_ctx.exec_state.clone()
        };
        exec_state.dispatch();
    }

    pub fn flat_priority(&self) -> u32 {
        self.0.flat_priority
    }
}

#[derive(Debug, Display, Error)]
#[display("Found a fugitive thread not boiund to the expected core")]
struct FugitiveThread;
