use derive_more::From;
use derive_where::derive_where;
use qapi::caps::CapError;
use qapi::caps::sync_ipc::{ExceptionAbi, StandardAbi};
use qapi::syscall::ops::introspect::SyncCallInspect;

use crate::arch::{InvokeAbi, System};
use crate::caps::Resources;
use crate::kmem::KPtr;
use crate::thread::ThreadExecCtx;

#[derive(Debug, Default, From)]
pub enum AnyAbi {
    #[default]
    NoReturn,
    Standard(StandardAbi),
    Exception(ExceptionAbi),
}

impl AnyAbi {
    pub fn ret_to<S: System>(
        &self,
        callee_ctx: &S::IrqCtx,
        caller_ctx: &S::ExecState,
    ) -> Result<(), CapError>
    where
        StandardAbi: InvokeAbi<S>,
        ExceptionAbi: InvokeAbi<S>,
    {
        match self {
            AnyAbi::NoReturn => Err(CapError::SyncRetOnNoRetAbi),
            AnyAbi::Standard(standard_abi) => {
                standard_abi.ret_passthrough(callee_ctx, caller_ctx);
                Ok(())
            }
            AnyAbi::Exception(exception_abi) => {
                exception_abi.ret_passthrough(callee_ctx, caller_ctx);
                Ok(())
            }
        }
    }
}

#[derive_where(Debug, Clone; Abi)]
pub struct SyncCall<S: System, Abi = StandardAbi> {
    comp: KPtr<Resources<S>>,
    entry: usize,
    abi: Abi,
}

impl<S: System, Abi> SyncCall<S, Abi> {
    pub const fn new(comp: KPtr<Resources<S>>, entry: usize, abi: Abi) -> Self {
        Self { comp, entry, abi }
    }

    pub fn introspect(&self) -> SyncCallInspect {
        SyncCallInspect
    }

    pub const fn resources(&self) -> &KPtr<Resources<S>> {
        &self.comp
    }

    pub fn create_invocation(self, ctx: &S::IrqCtx) -> ThreadExecCtx<S>
    where
        Abi: InvokeAbi<S> + Into<AnyAbi>,
    {
        let Self { comp, entry, abi } = self;
        let xstate = abi.invoke_passthrough(ctx, entry);
        ThreadExecCtx::new(xstate, comp, abi)
    }

    pub const fn entry(&self) -> usize {
        self.entry
    }
}
