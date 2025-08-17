use super::CapError;

#[cfg(target_arch = "x86_64")]
pub const PAGE_SIZE: usize = 4096;
pub const SLOT_SIZE: usize = 64;

pub const NUM_SLOTS: usize = PAGE_SIZE / SLOT_SIZE;

#[derive(Debug, Copy, Clone)]
pub struct SlotId<const COUNT: usize>(usize);

pub type SysSlot = SlotId<NUM_SLOTS>;

impl<const COUNT: usize> SlotId<COUNT> {
    pub const fn new(value: usize) -> Result<Self, CapError> {
        if value < COUNT {
            Ok(Self(value))
        } else {
            Err(CapError::InvalidTableSlot)
        }
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }
}

impl<const COUNT: usize> From<SlotId<COUNT>> for usize {
    fn from(value: SlotId<COUNT>) -> Self {
        value.0
    }
}

impl<const COUNT: usize> TryFrom<usize> for SlotId<COUNT> {
    type Error = CapError;
    fn try_from(value: usize) -> Result<Self, CapError> {
        Self::new(value)
    }
}
