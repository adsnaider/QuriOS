use core::{arch::asm, cell::RefCell};

use arch::{mem::Page, system, ArchSystem, GsBase, KernelGsBase, System};
use serial::sdbg;

use crate::{kmem::KPtr, pmo::PhysAddrExt, retyping::KernelFrame, thread::Thread};

#[derive(Debug, Default)]
#[repr(C)]
pub struct CoreLocalData {
    self_ptr: *mut Self,
    pub current_thread: RefCell<Option<KPtr<Thread<ArchSystem>>>>,
}

impl CoreLocalData {
    pub fn init(frame: KernelFrame) {
        const {
            assert!(size_of::<CoreLocalData>() <= Page::SIZE);
            assert!(Page::SIZE % align_of::<CoreLocalData>() == 0);
        }
        let frame = frame.into_raw();
        let addr = frame.addr().to_virtual();
        system().set_core_data(addr);

        let addr = addr.as_mut_ptr();
        let this = Self {
            self_ptr: addr,
            ..Default::default()
        };
        sdbg!(&this);
        // SAFETY: Address is valid and effectively leaked.
        unsafe { core::ptr::write_volatile(addr, this) };
    }

    pub fn get() -> &'static Self {
        // SAFETY: We only give shared references
        unsafe { &*Self::get_ptr() }
    }

    pub unsafe fn get_mut() -> &'static mut Self {
        // SAFETY: Precondition
        unsafe { &mut *Self::get_ptr_mut() }
    }

    pub fn get_ptr_mut() -> *mut Self {
        let ptr: *mut Self;
        unsafe {
            asm!("mov {}, qword ptr gs:[0]", lateout(reg) ptr);
        }
        log::debug!("HERE 2");
        debug_assert!(!ptr.is_null());
        ptr
    }

    pub fn get_ptr() -> *const Self {
        Self::get_ptr_mut() as *const Self
    }
}
