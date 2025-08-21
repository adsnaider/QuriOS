use x86_64::structures::paging::PhysFrame;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct Frame {
    base: PhysAddr,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct UnalignedAddress;

impl Frame {
    pub fn from_start_address(base: PhysAddr) -> Self {
        Self::try_from_start_address(base).unwrap()
    }

    pub fn base(&self) -> PhysAddr {
        self.base
    }

    pub fn from_index(index: u64) -> Result<Self, BadAddress> {
        let start = index * Self::SIZE;
        Ok(Self::from_start_address(PhysAddr::try_new(start)?))
    }

    pub fn try_from_start_address(base: PhysAddr) -> Result<Self, UnalignedAddress> {
        if base.as_u64() % Self::SIZE != 0 {
            return Err(UnalignedAddress);
        }
        Ok(Self { base })
    }

    pub fn within_frame(addr: PhysAddr) -> Self {
        let base = PhysAddr::new(addr.as_u64() & !(Self::SIZE - 1));
        Self { base }
    }

    pub fn next(&self) -> Self {
        Self {
            base: PhysAddr::new(self.base.as_u64() + Self::SIZE),
        }
    }

    pub fn addr(&self) -> PhysAddr {
        self.base
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct PhysAddr(u64);

impl core::fmt::Debug for PhysAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PhysAddr({:#X})", self.0)
    }
}

#[derive(Debug)]
pub struct BadAddress;

impl PhysAddr {
    pub const fn new(addr: u64) -> Self {
        match Self::try_new(addr) {
            Ok(addr) => addr,
            Err(_) => panic!("Invalid Physical Address: Must be up to 52 bits"),
        }
    }

    pub const fn try_new(addr: u64) -> Result<Self, BadAddress> {
        if Self::new_truncate(addr).0 == addr {
            Ok(Self(addr))
        } else {
            Err(BadAddress)
        }
    }

    pub const fn new_truncate(addr: u64) -> Self {
        Self(addr % (1 << 52))
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl From<Frame> for PhysFrame {
    fn from(value: Frame) -> Self {
        // SAFETY: Transparent representation
        unsafe { core::mem::transmute(value) }
    }
}

impl From<qapi::mem::Frame> for Frame {
    fn from(value: qapi::mem::Frame) -> Self {
        Self::from_start_address(PhysAddr::new(value.base()))
    }
}
