use derive_where::derive_where;
use qapi::syscall::ops::introspect::IntrospectResult;
use qapi::types::UserPtr;

use crate::arch::System;
use crate::caps::Resources;

#[derive_where(Debug, Clone)]
pub struct SyncCall<S: System> {
    comp: Resources<S>,
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
    pub const fn new(comp: Resources<S>, entry: usize) -> Self {
        Self { comp, entry }
    }

    pub fn introspect(&self) -> IntrospectResult {
        IntrospectResult::SyncCall
    }

    pub const fn resources(&self) -> &Resources<S> {
        &self.comp
    }

    pub const fn entry(&self) -> usize {
        self.entry
    }
}
