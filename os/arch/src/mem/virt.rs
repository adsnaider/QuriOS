use derive_more::{Display, Error};

use super::{HIGHER_HALF, UNTYPED_MEMORY_OFFSET};

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct Page {
    start_address: VirtAddr,
}

#[repr(transparent)]
#[derive(derive_more::Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
#[debug("VirtAddr({:#X?})", self.0)]
pub struct VirtAddr(pub(crate) usize);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Display, Error)]
#[display("The address isn't aligned to the page boundary")]
pub struct Unaligned;

impl Page {
    pub const fn from_start_address(addr: VirtAddr) -> Self {
        match Self::try_from_start_address(addr) {
            Err(Unaligned) => panic!("Unaligned start address"),
            Ok(this) => this,
        }
    }

    pub const fn try_from_start_address(addr: VirtAddr) -> Result<Self, Unaligned> {
        if addr.as_usize() % Self::SIZE != 0 {
            return Err(Unaligned);
        }
        Ok(Self {
            start_address: addr,
        })
    }

    pub fn containing_address(addr: VirtAddr) -> Self {
        let addr = VirtAddr::new((addr.as_usize() / Self::SIZE) * Self::SIZE);
        Self::from_start_address(addr)
    }

    pub fn base(&self) -> VirtAddr {
        self.start_address
    }

    /// Returns the page defined by `base = index * Self::SIZE`
    pub const fn from_index(index: usize) -> Result<Self, BadVirtAddr> {
        match VirtAddr::try_new(index * Self::SIZE) {
            Ok(addr) => Ok(Self::from_start_address(addr)),
            Err(e) => Err(e),
        }
    }

    /// Returns the index of this page (inverse of `from_index`)
    pub const fn index(&self) -> usize {
        self.start_address.as_usize() / Self::SIZE
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct BadVirtAddr;

impl VirtAddr {
    pub const fn new(addr: usize) -> Self {
        match Self::try_new(addr) {
            Ok(addr) => addr,
            Err(_) => panic!("Invalid address: Must be 48-bit sign-extended"),
        }
    }

    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new(ptr as usize)
    }

    pub const fn try_new(addr: usize) -> Result<Self, BadVirtAddr> {
        if Self::new_truncate(addr).0 == addr {
            Ok(Self::new_truncate(addr))
        } else {
            Err(BadVirtAddr)
        }
    }

    pub const fn new_truncate(addr: usize) -> Self {
        Self(((addr << 16) as i64 >> 16) as usize)
    }

    pub const fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    pub const fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    pub const fn as_usize(&self) -> usize {
        self.0
    }

    pub const fn zero() -> Self {
        Self::new(0)
    }

    pub const fn memory_segment(&self) -> MemorySegment {
        match self.0 {
            ..UNTYPED_MEMORY_OFFSET => MemorySegment::User,
            UNTYPED_MEMORY_OFFSET..HIGHER_HALF => MemorySegment::Untyped,
            HIGHER_HALF.. => MemorySegment::Kernel,
        }
    }

    pub const fn is_higher_half(&self) -> bool {
        self.0 >= 0xFFFF_8000_0000_0000
    }

    pub const fn is_lower_half(&self) -> bool {
        !self.is_higher_half()
    }

    pub const fn is_kernel(&self) -> bool {
        self.is_higher_half()
    }

    pub const fn is_user(&self) -> bool {
        self.0 < UNTYPED_MEMORY_OFFSET
    }

    pub const fn is_untyped_region(&self) -> bool {
        self.is_lower_half() && self.0 >= UNTYPED_MEMORY_OFFSET
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MemorySegment {
    Kernel,
    User,
    Untyped,
}
