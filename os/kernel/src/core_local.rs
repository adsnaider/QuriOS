use core::{
    arch::asm,
    mem::{align_of, size_of},
};

use arch::{mem::Page, system, ArchSystem, System};
use serial::sdbg;

use crate::{
    kmem::KPtr, pmo::PhysAddrExt, retyping::KernelFrame, syscall::syscall_token::SyscallToken,
    thread::Thread,
};

#[derive(Debug, Default)]
#[repr(C)]
pub struct CoreLocalData {
    self_ptr: *mut Self,
    pub current_thread: Option<KPtr<Thread<ArchSystem>>>,
}

#[allow(unused)]
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
        unsafe { core::ptr::write(addr, this) };
    }

    pub fn get<'a, 'brand>(_token: &'a SyscallToken<'brand>) -> &'a Self {
        // SAFETY: Access to token guarantees shared borrow semantics here.
        unsafe { &*Self::get_ptr() }
    }

    pub fn get_mut<'a, 'brand>(_token: &'a mut SyscallToken<'brand>) -> &'a mut Self {
        // SAFETY: Access to token guarantees mutable borrow semantics here.
        unsafe { &mut *Self::get_ptr_mut() }
    }

    pub fn get_ptr_mut() -> *mut Self {
        let ptr: *mut Self;
        // SAFETY: The first qword in the gs base will be the self-referencing pointer.
        unsafe {
            asm!("mov {}, qword ptr gs:[0]", lateout(reg) ptr);
        }
        debug_assert!(!ptr.is_null());
        ptr
    }

    pub fn get_ptr() -> *const Self {
        Self::get_ptr_mut() as *const _
    }
}
