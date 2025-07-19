use arch::System;
use derive_where::derive_where;
use qapi::types::UserPtr;

use crate::caps::Resources;

#[derive_where(Debug, Clone)]
pub struct SyncCall<S: System> {
    comp: Resources<S>,
    entry: UserPtr<()>,
}

#[derive(Debug, Clone)]
pub struct SyncRet;
