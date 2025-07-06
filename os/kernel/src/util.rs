use arch::mem::{PhysAddr, VirtAddr};

use crate::PMO;

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct Pmo {
    base: VirtAddr,
}

impl Pmo {
    /// Constructs a physical memory offset with the given virtual address base
    ///
    /// # Safety
    ///
    /// The base address must accurately convey the virtual address of all physical memory
    pub unsafe fn new(base: VirtAddr) -> Self {
        Self { base }
    }

    #[cfg(target_pointer_width = "64")]
    pub fn phys_to_virt(&self, physical: PhysAddr) -> VirtAddr {
        // as SAFETY: pointer width is 64 bits.
        let virt = self.base.as_usize() + physical.as_u64() as usize;
        VirtAddr::new(virt)
    }

    /// # Safety
    ///
    /// The virtual address must have been created with `to_virtual`
    #[cfg(target_pointer_width = "64")]
    pub unsafe fn virt_to_phys(&self, addr: VirtAddr) -> PhysAddr {
        // as SAFETY: pointer width is 64 bits.
        let paddr = addr.as_ptr::<()>() as u64 - self.base.as_ptr::<()>() as u64;
        PhysAddr::new(paddr)
    }
}

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
