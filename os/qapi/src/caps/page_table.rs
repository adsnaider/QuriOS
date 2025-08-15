use derive_more::{From, Into};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use super::CapId;

#[repr(transparent)]
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    From,
    Into,
    KnownLayout,
    IntoBytes,
    FromBytes,
    Immutable,
)]
pub struct PageTableCap(CapId);

impl PageTableCap {
    pub const fn new(cap: CapId) -> Self {
        Self(cap)
    }

    pub const fn cap(&self) -> CapId {
        self.0
    }
}
