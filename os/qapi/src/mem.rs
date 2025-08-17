pub mod phys;
pub mod virt;

use bitflags::bitflags;
pub use phys::Frame;
pub use virt::Page;

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct PageFlags: u64 {
        const PRESENT = 1;
        const READABLE = 1 << 1;
        const WRITABLE = 1 << 2;
        const EXECUTABLE = 1 << 3;
    }
}
