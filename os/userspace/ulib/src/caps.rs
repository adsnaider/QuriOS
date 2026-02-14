use qapi::caps::{CapId, SysSlot};

#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct CPath(CapId);

impl CPath {
    pub const fn empty() -> Self {
        Self(CapId::new(0))
    }

    pub const fn new(cap: CapId) -> Self {
        Self(cap)
    }

    pub const fn push(self, slot: SysSlot) -> Self {
        let mut cap = self.0.value() << SysSlot::slot_bits();
        cap |= slot.as_usize() as u32;
        Self(CapId::new(cap))
    }

    pub const fn split_end(self) -> (CPath, SysSlot) {
        let cap = self.0.value();
        let slot = cap & (SysSlot::slot_count() - 1) as u32;
        let cpath = cap >> SysSlot::slot_bits();
        (CPath(CapId::new(cpath)), SysSlot::new(slot as usize))
    }

    pub const fn cap(&self) -> CapId {
        self.0
    }
}
