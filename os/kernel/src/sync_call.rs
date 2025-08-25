use derive_where::derive_where;
use qapi::syscall::ops::introspect::IntrospectResult;

use crate::arch::{InvokeAbi, System};
use crate::caps::Resources;
use crate::kmem::KPtr;

#[derive(Debug, Default, Copy, Clone)]
pub struct CallAbi;

#[derive_where(Debug, Clone; Abi)]
pub struct SyncCall<S: System, Abi = CallAbi> {
    comp: KPtr<Resources<S>>,
    entry: usize,
    abi: Abi,
}

#[derive(Debug, Clone)]
pub struct SyncRet;

impl SyncRet {
    pub fn introspect(&self) -> IntrospectResult {
        IntrospectResult::SyncRet
    }
}

impl<S: System, Abi> SyncCall<S, Abi> {
    pub const fn new(comp: KPtr<Resources<S>>, entry: usize, abi: Abi) -> Self {
        Self { comp, entry, abi }
    }

    pub fn introspect(&self) -> IntrospectResult {
        IntrospectResult::SyncCall
    }

    pub const fn resources(&self) -> &KPtr<Resources<S>> {
        &self.comp
    }

    pub fn create_invocation(self, ctx: &S::IrqCtx) -> (KPtr<Resources<S>>, S::ExecState, S::RetAbi)
    where
        Abi: InvokeAbi<S>,
    {
        let Self { comp, entry, abi } = self;
        let (xstate, sret) = abi.new_invocation(entry, ctx);
        (comp, xstate, sret)
    }

    pub const fn entry(&self) -> usize {
        self.entry
    }
}
