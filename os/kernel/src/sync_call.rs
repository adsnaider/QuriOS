use derive_where::derive_where;
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
