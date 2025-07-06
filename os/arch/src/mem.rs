mod phys;
mod virt;

pub use phys::{Frame, PhysAddr};
pub use virt::{Page, VirtAddr};

pub trait Addrspace {}
