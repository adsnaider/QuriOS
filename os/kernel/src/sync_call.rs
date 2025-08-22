use derive_where::derive_where;
use qapi::syscall::ops::introspect::IntrospectResult;

use crate::arch::System;
use crate::caps::Resources;
use crate::kmem::KPtr;

#[derive_where(Debug, Clone)]
pub struct SyncCall<S: System> {
    comp: KPtr<Resources<S>>,
    entry: usize,
}

#[derive(Debug, Clone)]
pub struct SyncRet;

impl SyncRet {
    pub fn introspect(&self) -> IntrospectResult {
        IntrospectResult::SyncRet
    }
}

impl<S: System> SyncCall<S> {
    pub const fn new(comp: KPtr<Resources<S>>, entry: usize) -> Self {
        Self { comp, entry }
    }

    pub fn introspect(&self) -> IntrospectResult {
        IntrospectResult::SyncCall
    }

    pub const fn resources(&self) -> &KPtr<Resources<S>> {
        &self.comp
    }

    pub fn into_parts(self) -> (KPtr<Resources<S>>, usize) {
        let SyncCall { comp, entry } = self;
        (comp, entry)
    }

    pub const fn entry(&self) -> usize {
        self.entry
    }
}
