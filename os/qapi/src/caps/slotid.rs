use super::CapError;

#[cfg(target_arch = "x86_64")]
pub const PAGE_SIZE: usize = 4096;
pub const SLOT_SIZE: usize = 64;

pub const NUM_SLOTS: usize = PAGE_SIZE / SLOT_SIZE;

#[derive(Debug, Copy, Clone)]
pub struct SlotId<const COUNT: usize>(usize);

pub type SysSlot = SlotId<NUM_SLOTS>;

impl<const COUNT: usize> SlotId<COUNT> {
    pub const fn slot_count() -> usize {
        NUM_SLOTS
    }

    pub const fn slot_bits() -> usize {
        assert!(
            COUNT.is_power_of_two(),
            "Unexpected syslot should be a power of 2"
        );
        assert!(COUNT != 0, "Unexpected syslot should be a power of 2");
        COUNT.ilog2() as usize
    }

    pub const fn zero() -> Self {
        Self::new(0)
    }

    pub const fn try_new(value: usize) -> Result<Self, CapError> {
        if value < COUNT {
            Ok(Self(value))
        } else {
            Err(CapError::InvalidTableSlot)
        }
    }

    pub const fn new(value: usize) -> Self {
        match Self::try_new(value) {
            Ok(value) => value,
            Err(_) => panic!("Invalid slot"),
        }
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn checked_add(self, rhs: isize) -> Option<Self> {
        let val = self.0.checked_add_signed(rhs);
        let Some(val) = val else {
            return None;
        };
        if val >= COUNT { None } else { Some(Self(val)) }
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
        Self::try_new(value)
    }
}
