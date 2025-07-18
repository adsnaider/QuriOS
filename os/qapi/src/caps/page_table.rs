use derive_more::{From, Into};

use super::CapId;

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into)]
pub struct Addrspace(CapId);
