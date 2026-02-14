use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::sync::atomic::{AtomicU32, Ordering, fence};

use derive_more::{Display, Error};
use derive_where::derive_where;
use heapless::Vec;
use qapi::caps::sync_ipc::{ExceptionAbi, StandardAbi};
use qapi::caps::{CapError, CapId};
use qapi::syscall::ops::introspect;
use qapi::syscall::ops::sync_ipc::SyncRetOp;

use crate::arch::mem::Addrspace;
use crate::arch::{ArchSystem, ExecState, InvokeAbi, System};
use crate::caps::{CapRef, CapTable, ExceptionHandler, Resources};
use crate::core_local::core_cell::{BindError, CoreGuard, Lock, UnbindError};
use crate::core_local::{CORE_LOCAL_CURRENT_THREAD, CoreCell};
use crate::kmem::KPtr;
use crate::sync_call::AnyAbi;
use crate::util::{OptionExt as _, ResultExt};

pub type CoreLocalThread = Lock<Option<KPtr<Thread<ArchSystem>>>>;

#[derive_where(Debug)]
pub struct ThreadExecStack<S: System> {
    exec_stack: Vec<ThreadExecCtx<S>, 16>,
    run_state: RunState,
}

impl<S: System> ThreadExecStack<S> {
    pub const fn new(exec_stack: Vec<ThreadExecCtx<S>, 16>) -> Self {
        Self {
            exec_stack,
            run_state: RunState::Runnable,
        }
    }
    pub fn active(&self) -> &ThreadExecCtx<S> {
        self.exec_stack.last().unwrap()
    }

    pub fn active_mut(&mut self) -> &mut ThreadExecCtx<S> {
        self.exec_stack.last_mut().unwrap()
    }

    pub fn is_full(&self) -> bool {
        self.exec_stack.is_full()
    }

    pub fn is_base(&self) -> bool {
        self.exec_stack.len() == 1
    }

    pub fn push(&mut self, ctx: ThreadExecCtx<S>) -> Result<(), ThreadExecCtx<S>> {
        self.exec_stack.push(ctx)
    }

    pub fn pop(&mut self) -> Option<ThreadExecCtx<S>> {
        if self.is_base() {
            None
        } else {
            Some(self.exec_stack.pop().unwrap())
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

    pub fn resources(&self) -> &KPtr<Resources<S>> {
        &self.resources
    }

    pub fn exception_handler(&self) -> &ExceptionHandler {
        self.resources.exception_handler()
    }
}

#[derive_where(Debug)]
pub struct Thread<S: System> {
    execution_stack: CoreCell<Lock<ThreadExecStack<S>>>,
    flat_priority: u32,
    signals: AtomicU32,
    parent: Option<KPtr<Self>>,
}

impl Thread<ArchSystem> {
    pub fn current() -> &'static CoreLocalThread {
        &CORE_LOCAL_CURRENT_THREAD
    }

    fn replace_current(new: KPtr<Self>) -> Option<KPtr<Self>> {
        log::debug!("Replacing current thread: new: {new:?}");
        // SAFETY: Affinity should be set before replacing thread.
        unsafe { new.verify_affinity().unwrap_debug() };
        let old = CORE_LOCAL_CURRENT_THREAD.replace(Some(new)).unwrap();
        log::debug!("old: {old:?}");
        old
    }

    pub fn with_current<F, T>(fun: F) -> T
    where
        F: FnOnce(&Thread<ArchSystem>) -> T,
    {
        Self::current().locked(|current| {
            // SAFETY: This should only be called after the initial kernel dispatch.
            let current = unsafe { current.as_mut().unwrap_debug() };
            // SAFETY: Affinity should be set on current thread
            unsafe { current.verify_affinity().unwrap_debug() };
            fun(current)
        })
    }

    pub fn get_current() -> Option<KPtr<Self>> {
        Self::current().locked(|current| current.clone())
    }

    pub fn kinit_dispatch(this: KPtr<Self>) -> Result<DispatchToken<ArchSystem>, BindError> {
        DispatchToken::new(this, None)
    }

    pub fn dispatch(
        this: KPtr<Self>,
        irq_ctx: <ArchSystem as System>::IrqCtx,
    ) -> Result<DispatchToken<ArchSystem>, CapError> {
        Ok(DispatchToken::new(this, Some(irq_ctx))?)
    }

    pub fn priority_dispatch(
        next: &KPtr<Self>,
        ctx: <ArchSystem as System>::IrqCtx,
    ) -> Result<Option<DispatchToken<ArchSystem>>, CapError> {
        let dispatch = Self::with_current(|current| next.flat_priority() > current.flat_priority());
        if dispatch {
            Ok(Some(Self::dispatch(next.clone(), ctx)?))
        } else {
            Ok(None)
        }
    }

    pub fn notify(
        this: &KPtr<Self>,
        signals: u32,
        irq_ctx: <ArchSystem as System>::IrqCtx,
    ) -> Result<Option<DispatchToken<ArchSystem>>, CapError> {
        log::trace!("Notifying thread: sigs {signals:#b}");
        let old_sigs = this.signals.fetch_or(signals, Ordering::Relaxed);
        fence(Ordering::SeqCst);
        let signals = old_sigs | signals;
        if signals != 0 {
            match Thread::priority_dispatch(this, irq_ctx) {
                Ok(Some(mut token)) => {
                    let state = this.execution_stack().locked(|s| s.run_state);
                    match state {
                        RunState::Runnable => {
                            // Thread isn't actually waiting for signals, so let the scheduler decide if it should be run
                            Ok(None)
                        }
                        RunState::SigBlocked => {
                            this.execution_stack()
                                .locked(|s| s.run_state = RunState::Runnable);
                            let sigs = this.signals.swap(0, Ordering::Relaxed);
                            token.set_out_reg(sigs as usize);
                            Ok(Some(token))
                        }
                    }
                }
                Ok(None) => Ok(None),
                Err(CapError::ThreadBoundToOtherCore) => Ok(None),
                Err(e) => Err(e),
            }
        } else {
            Ok(None)
        }
    }

    pub fn sig_wait(
        this: KPtr<Self>,
        irq_ctx: <ArchSystem as System>::IrqCtx,
    ) -> Result<SigWaitResult<ArchSystem>, CapError> {
        let signals = this.signals.swap(0, Ordering::Relaxed);
        if signals != 0 {
            Ok(SigWaitResult::Signalled(signals))
        } else {
            let Some(parent) = &this.parent else {
                return Err(CapError::SigWaitNoParent);
            };

            this.execution_stack()
                .locked(|s| s.run_state = RunState::SigBlocked);
            let dispatch = Thread::dispatch(parent.clone(), irq_ctx)?;
            // If we, the waiter (W), are racing against a notifier N, a load observing a zero here
            // must be guaranteed to happen  before N attempts the priority dispatch. Acquire/Release
            // isn't sufficient here. There must be a total  order defined between the dispatch
            // (bind/unbind) atomic and the signal such that for W:  load 0 -> unbind -> load 0, and for N:
            // store sigs -> bind.
            //
            // The conflicting situation with weaker orderings is the following
            // W observes: Load 0 -> unbind -> load 0
            // N observes: Store sigs -> bind fails due to already bound
            // For this to happen, the order of atomics would have to be load 0 (W) -> bind fails (N) -> load 0 (W) -> store sigs (N),
            // but this ordering conflicts with N's perspected which is store sigs -> bind fails.
            //
            // The only way this may happen with SeqCst is if there exists another core that binds to the thread first which would be
            // accepted.
            fence(Ordering::SeqCst);
            let signals = this.signals.load(Ordering::Relaxed);
            if signals == 0 {
                Ok(SigWaitResult::Blocked(dispatch))
            } else {
                match this.bind() {
                    Ok(()) => {
                        let state = this.execution_stack().locked(|s| s.run_state);
                        match state {
                            RunState::Runnable => {
                                // In the interim another core bound to this and somehow ended up doing something like
                                // yielding. Jump back to the scheduler (parent)...
                                Ok(SigWaitResult::Blocked(dispatch))
                            }
                            RunState::SigBlocked => {
                                this.execution_stack()
                                    .locked(|s| s.run_state = RunState::Runnable);
                                let signals = this.signals.swap(0, Ordering::Relaxed);
                                let mut alternate_dispatch = SameThreadDispatchToken::new(
                                    this.execution_stack().lock().active().exec_state.clone(),
                                );
                                alternate_dispatch.set_out_reg(signals as usize);
                                Ok(SigWaitResult::Redispatch(dispatch.bail(alternate_dispatch)))
                            }
                        }
                    }
                    Err(BindError::Bound { affinity: _ }) => Ok(SigWaitResult::Blocked(dispatch)),
                }
            }
        }
    }
}

impl<S: System> Thread<S> {
    pub fn new(
        exec_state: S::ExecState,
        comp: KPtr<Resources<S>>,
        priority: u32,
        parent: Option<KPtr<Self>>,
    ) -> Self {
        let mut ctx = Vec::new();
        ctx.push(ThreadExecCtx {
            exec_state,
            resources: comp,
            abi: Default::default(),
        })
        .unwrap();
        Self {
            execution_stack: CoreCell::new(Lock::new(ThreadExecStack::new(ctx))),
            flat_priority: priority,
            signals: AtomicU32::new(0),
            parent,
        }
    }

    pub fn introspect(&self) -> introspect::ThreadInspect {
        introspect::ThreadInspect
    }

    pub fn bind(&self) -> Result<(), BindError> {
        self.execution_stack.bind()?;
        self.verify_affinity().unwrap();
        Ok(())
    }

    pub fn unbind(&self) -> Result<(), UnbindError> {
        self.execution_stack.unbind()
    }

    fn verify_affinity(&self) -> Result<(), FugitiveThread> {
        if self.execution_stack.is_locally_bound() {
            Ok(())
        } else {
            Err(FugitiveThread)
        }
    }

    pub fn flat_priority(&self) -> u32 {
        self.flat_priority
    }

    pub fn resources(&self) -> KPtr<Resources<S>> {
        self.execution_stack().lock().active().resources().clone()
    }

    pub fn execution_stack(&self) -> CoreGuard<'_, Lock<ThreadExecStack<S>>> {
        self.execution_stack.try_get().unwrap()
    }

    pub fn invoke(
        &self,
        invocation: ThreadExecCtx<S>,
        ctx: S::IrqCtx,
    ) -> Result<SameThreadDispatchToken<S>, CapError> {
        let curr_ctx = self.execution_stack();
        let mut curr_ctx = curr_ctx.lock();
        if curr_ctx.is_full() {
            return Err(CapError::SyncInvokeLimit);
        }
        curr_ctx.active_mut().exec_state.save(&ctx);
        invocation.resources.addrspace().activate();
        let xstate = invocation.exec_state.clone();
        curr_ctx.push(invocation).unwrap();
        Ok(SameThreadDispatchToken::new(xstate))
    }

    pub fn sync_ret(
        &self,
        _args: SyncRetOp,
        callee_ctx: S::IrqCtx,
    ) -> Result<SameThreadDispatchToken<S>, CapError>
    where
        StandardAbi: InvokeAbi<S>,
        ExceptionAbi: InvokeAbi<S>,
    {
        let thread_ctx = self.execution_stack();
        let mut thread_ctx = thread_ctx.lock();
        if thread_ctx.is_base() {
            return Err(CapError::SyncRetLimit);
        }

        let ThreadExecCtx { abi, .. } = thread_ctx.pop().unwrap();
        let caller_ctx = thread_ctx.active_mut();
        abi.ret_to(&callee_ctx, &caller_ctx.exec_state)?;
        caller_ctx.resources.addrspace().activate();
        Ok(SameThreadDispatchToken::new(caller_ctx.exec_state.clone()))
    }

    pub fn exception_handler(&self) -> ExceptionHandler {
        self.execution_stack()
            .lock()
            .active()
            .resources()
            .exception_handler()
            .clone()
    }

    pub fn get_cap(&self, cap: CapId) -> Option<CapRef<S>> {
        self.execution_stack().lock().active().get_cap(cap)
    }
}

pub enum SigWaitResult<S: System> {
    Blocked(DispatchToken<S>),
    Signalled(u32),
    Redispatch(SameThreadDispatchToken<S>),
}

#[must_use]
#[derive(Debug)]
pub struct DispatchToken<S: System> {
    next: KPtr<Thread<S>>,
    exec_state: S::ExecState,
}

impl DispatchToken<ArchSystem> {
    pub fn new(
        next: KPtr<Thread<ArchSystem>>,
        ctx: Option<<ArchSystem as System>::IrqCtx>,
    ) -> Result<Self, BindError> {
        next.bind()?;

        let exec_state;
        {
            let exec_stack = next.execution_stack();
            let exec_stack = exec_stack.lock();
            exec_stack.active().addrspace().activate();
            exec_state = exec_stack.active().exec_state.clone();
            if let Some(ref previous) = *Thread::current().lock() {
                {
                    let prev_ctx = previous.execution_stack();
                    let prev_ctx = prev_ctx.lock();
                    prev_ctx.active().exec_state.save(
                        ctx.as_ref()
                            .expect("Missing IRQ context past initial dispatch"),
                    );
                }
                previous
                    .unbind()
                    .expect("Active references to previous thread preventing unbinding!");
                // At this point, any other core may come in and execute the previous thread which is fine as it was saved above and it won't be used further
            };
        }
        Ok(Self { next, exec_state })
    }

    pub fn dispatch(self) -> ! {
        let this = ManuallyDrop::new(self);
        // SAFETY: Manually drop protects this ptr read.
        let next = unsafe { core::ptr::read(&this.next) };
        Thread::replace_current(next);
        this.exec_state.dispatch();
    }

    // Assumes that D is another dispatcher and bails the current one
    //
    // In general, if we unbound from the thread (which was done in ::new), we are not
    // allowed to return back to the caller without a dispatcher as the current thread may have changed (in another core). For that reason, this is a soft-check that another dispatcher is available to return to the other
    // thread's execution. It's really hard to define a hard check for this though since it's
    // easy enough to ignore the dispatcher altogether.
    pub fn bail<D>(self, other_dispatcher: D) -> D {
        core::mem::forget(self);
        other_dispatcher
    }

    pub fn set_out_reg(&mut self, value: usize) {
        self.exec_state.set_out_reg(value);
    }
}

impl<S: System> Drop for DispatchToken<S> {
    fn drop(&mut self) {
        panic!("Dropping the dispatch token isn't allowed");
    }
}

#[must_use]
#[derive(Debug)]
pub struct SameThreadDispatchToken<S: System> {
    exec_state: S::ExecState,
}

impl<S: System> SameThreadDispatchToken<S> {
    pub fn new(exec_state: S::ExecState) -> Self {
        Self { exec_state }
    }
}
impl SameThreadDispatchToken<ArchSystem> {
    pub fn dispatch(self) -> ! {
        self.exec_state.dispatch();
    }

    pub fn set_out_reg(&mut self, value: usize) {
        self.exec_state.set_out_reg(value);
    }

    pub fn bail(self) {
        core::mem::forget(self);
    }
}

impl<S: System> Drop for SameThreadDispatchToken<S> {
    fn drop(&mut self) {
        panic!("Dropping the dispatch token isn't allowed");
    }
}

#[derive(Debug, Display, Error)]
#[display("Found a fugitive thread not boiund to the expected core")]
pub struct FugitiveThread;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum RunState {
    Runnable,
    SigBlocked,
}
