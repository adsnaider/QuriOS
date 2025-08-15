pub mod phys;
#[cfg(feature = "userspace")]
pub mod ulib;
pub mod virt;

pub use phys::Frame;
pub use virt::Page;

use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct PageFlags: u64 {
        const PRESENT = 1;
        const READABLE = 1 << 1;
        const WRITABLE = 1 << 2;
        const EXECUTABLE = 1 << 3;
    }
}
