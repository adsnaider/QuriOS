use derive_more::{Debug, Display, Error};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
#[debug("Page({:#X})", self.base)]
pub struct Page {
    base: usize,
}

#[derive(Debug, Clone, Copy, Error, Display)]
pub enum InvalidPage {
    #[display("The frame base address is not aligned to a frame boundary")]
    Misaligned,
}

impl Page {
    #[cfg(target_arch = "x86_64")]
    pub const SIZE: usize = 4096;

    pub const fn try_new(start_addr: usize) -> Result<Self, InvalidPage> {
        if start_addr % Self::SIZE != 0 {
            return Err(InvalidPage::Misaligned);
        }
        Ok(Self { base: start_addr })
    }

    pub const fn from_index(index: usize) -> Self {
        match Self::try_new(index * Self::SIZE) {
            Ok(t) => t,
            Err(InvalidPage::Misaligned) => panic!("Unreachable panic: bad address: Misaligned"),
        }
    }

    pub const fn base(&self) -> usize {
        self.base
    }

    pub const fn index(&self) -> usize {
        self.base / Self::SIZE
    }
}

#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
#[cfg(target_arch = "x86_64")]
mod x86_64 {
    use derive_more::Into;

    use super::Page;

    impl Page {
        /// Returns the 9-bit level 1 page table index.
        #[inline]
        pub fn p1_index(self) -> PageTableOffset {
            PageTableOffset::new_truncate((self.base >> 12) as u16)
        }

        /// Returns the 9-bit level 2 page table index.
        #[inline]
        pub fn p2_index(self) -> PageTableOffset {
            PageTableOffset::new_truncate((self.base >> 12 >> 9) as u16)
        }

        /// Returns the 9-bit level 3 page table index.
        #[inline]
        pub fn p3_index(self) -> PageTableOffset {
            PageTableOffset::new_truncate((self.base >> 12 >> 9 >> 9) as u16)
        }

        /// Returns the 9-bit level 4 page table index.
        #[inline]
        pub fn p4_index(self) -> PageTableOffset {
            PageTableOffset::new_truncate((self.base >> 12 >> 9 >> 9 >> 9) as u16)
        }

        /// Returns the 9-bit level page table index.
        #[inline]
        pub fn page_table_index(self, level: PageTableLevel) -> PageTableOffset {
            PageTableOffset::new_truncate((self.base >> 12 >> ((level.level() - 1) * 9)) as u16)
        }
    }

    #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Into)]
    pub struct PageTableOffset(u16);

    #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
    pub struct PageTableLevel(u8);

    #[derive(Debug)]
    pub struct InvalidLevel;

    impl PageTableLevel {
        pub const fn new(level: u8) -> Self {
            match Self::try_new(level) {
                Ok(level) => level,
                Err(_) => panic!("Page table level must be within 1 and 4",),
            }
        }

        pub const fn try_new(level: u8) -> Result<Self, InvalidLevel> {
            if level < 1 || level > 4 {
                return Err(InvalidLevel);
            }
            Ok(Self(level))
        }

        pub const fn level(&self) -> u8 {
            self.0
        }

        pub const fn top() -> Self {
            Self(4)
        }

        pub const fn is_bottom(&self) -> bool {
            self.level() == 1
        }

        pub const fn lower(self) -> Option<Self> {
            match Self::try_new(self.level() - 1) {
                Ok(l) => Some(l),
                Err(_) => None,
            }
        }
    }

    #[derive(Debug)]
    pub enum PageTableOffsetError {
        OutOfBounds,
    }

    impl TryFrom<u16> for PageTableOffset {
        type Error = PageTableOffsetError;

        fn try_from(value: u16) -> Result<Self, Self::Error> {
            Self::new(value)
        }
    }

    impl TryFrom<usize> for PageTableOffset {
        type Error = PageTableOffsetError;

        fn try_from(value: usize) -> Result<Self, Self::Error> {
            Self::new(u16::try_from(value).map_err(|_| PageTableOffsetError::OutOfBounds)?)
        }
    }

    impl PageTableOffset {
        pub const fn new(offset: u16) -> Result<Self, PageTableOffsetError> {
            if offset < 512 {
                Ok(Self(offset))
            } else {
                Err(PageTableOffsetError::OutOfBounds)
            }
        }

        pub const fn is_lower_half(&self) -> bool {
            self.0 < 256
        }

        pub const fn new_truncate(addr: u16) -> Self {
            Self(addr % 512)
        }
    }

    impl From<PageTableOffset> for usize {
        fn from(value: PageTableOffset) -> Self {
            value.0 as usize
        }
    }
}
