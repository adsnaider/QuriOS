use derive_where::derive_where;
use qapi::syscall::ops::introspect::IntrospectResult;
use qapi::types::UserPtr;

use crate::arch::System;
use crate::caps::Resources;

#[derive_where(Debug, Clone)]
pub struct SyncCall<S: System> {
    _comp: Resources<S>,
    _entry: UserPtr<()>,
    _tag: usize,
}

#[derive(Debug, Clone)]
pub struct SyncRet;

impl SyncRet {
    pub fn introspect(&self) -> IntrospectResult {
        IntrospectResult::SyncRet
    }
}

impl<S: System> SyncCall<S> {
    pub fn introspect(&self) -> IntrospectResult {
        IntrospectResult::SyncCall
    }
}
