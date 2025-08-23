use core::marker::PhantomData;

use derive_where::derive_where;
use qapi::syscall::ops::introspect::IntrospectResult;

use crate::arch::System;
use crate::caps::Resources;
use crate::kmem::KPtr;

pub struct CallAbi;
pub struct ExceptionAbi;

#[derive_where(Debug, Clone)]
pub struct SyncCall<S: System, Abi = CallAbi> {
    comp: KPtr<Resources<S>>,
    entry: usize,
    _abi: PhantomData<Abi>,
}

#[derive(Debug, Clone)]
pub struct SyncRet;

impl SyncRet {
    pub fn introspect(&self) -> IntrospectResult {
        IntrospectResult::SyncRet
    }
}

impl<S: System, Abi> SyncCall<S, Abi> {
    pub const fn new(comp: KPtr<Resources<S>>, entry: usize) -> Self {
        Self {
            comp,
            entry,
            _abi: PhantomData,
        }
    }

    pub fn introspect(&self) -> IntrospectResult {
        IntrospectResult::SyncCall
    }

    pub const fn resources(&self) -> &KPtr<Resources<S>> {
        &self.comp
    }

    pub fn into_parts(self) -> (KPtr<Resources<S>>, usize) {
        let SyncCall { comp, entry, _abi } = self;
        (comp, entry)
    }

    pub const fn entry(&self) -> usize {
        self.entry
    }
}
