pub mod core_cell;
use core::cell::RefCell;
use core::mem::{align_of, size_of};

pub use core_cell::CoreCell;

use crate::arch::mem::Page;
use crate::arch::mem::core_local::CoreLocalData;
use crate::arch::{System, system};
use crate::pmo::PhysAddrExt;
use crate::retyping::KernelFrame;
use crate::thread::CurrentThread;

#[repr(C)]
pub struct KernelLocalStore {
    current_thread: CurrentThread,
    safe_buffer_lock: bool,
    core_id: u16,
}

impl KernelLocalStore {
    pub const fn new(core_id: u16) -> Self {
        Self {
            core_id,
            current_thread: RefCell::new(None),
            safe_buffer_lock: false,
        }
    }
}

macro_rules! get_core_local_data_impl {
    ($vis:vis $field:ident, $ty:ty) => {
        paste::paste! {
            #[allow(unused)]
            $vis const [<CORE_LOCAL_ $field:upper _OFF>]: usize = core::mem::offset_of!(crate::arch::mem::core_local::CoreLocalData<KernelLocalStore>, data.$field);
            // SAFETY: This is safe by construction since we get the offset with `offset_of` macro and this is the only place we are allowed to construct a CoreLocal type.
            $vis static [<CORE_LOCAL_ $field:upper >]: crate::arch::mem::core_local::CoreLocal<[<CORE_LOCAL_ $field:upper _OFF>], $ty> = unsafe { crate::arch::mem::core_local::CoreLocal::new() };
        }
    };
}

#[extend::ext]
pub impl CoreLocalData<KernelLocalStore> {
    fn init(frame: KernelFrame, core_id: u16) {
        const {
            assert!(size_of::<Self>() <= Page::SIZE);
            assert!(Page::SIZE % align_of::<Self>() == 0);
        }
        let frame = frame.into_raw();
        let addr = frame.addr().to_virtual();
        system().set_core_data(addr);

        let addr = addr.as_mut_ptr();
        let this = Self::new(addr, KernelLocalStore::new(core_id));
        // SAFETY: Address is valid and effectively leaked.
        unsafe { core::ptr::write(addr, this) };
    }
}

get_core_local_data_impl!(pub current_thread, CurrentThread);
get_core_local_data_impl!(pub safe_buffer_lock, bool);
get_core_local_data_impl!(pub core_id, u16);
