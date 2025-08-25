use crate::PMO;
use crate::arch::mem::{PhysAddr, VirtAddr};

#[extend::ext]
pub impl VirtAddr {
    unsafe fn to_physical(self) -> PhysAddr {
        // SAFETY: Precondition
        unsafe { PMO.virt_to_phys(self) }
    }
}

#[extend::ext]
pub impl PhysAddr {
    fn to_virtual(self) -> VirtAddr {
        PMO.phys_to_virt(self)
    }
}
