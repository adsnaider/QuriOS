use core::borrow::Borrow;
use core::fmt::Debug;

use exec::ExecState;
use mem::{Addrspace, VirtAddr};
use qapi::caps::CapError;
use sync::cell::AtomicOnceCell;

use crate::kmem::KPtr;

#[cfg(target_arch = "x86_64")]
mod x86_64_impl;

pub mod exec;
pub mod mem;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        pub type ArchSystem = x86_64_impl::X64Sys;
    } else {
        const _: () = const { panic!("Target architecture not supported") };
    }
}

static SYSTEM: AtomicOnceCell<ArchSystem> = AtomicOnceCell::new();
pub fn system() -> &'static ArchSystem {
    SYSTEM
        .get()
        .expect("System not fully initialized. Did you call `init`?")
}

/// Initializes the architecture-specific subsystem.
pub fn init() {
    SYSTEM
        .set(ArchSystem::init())
        .expect("Tried to initialize system twice")
}

/// Architecture-agnostic system management
///
/// This trait serves to provide the general interface between the kernel
/// and the architecture-specific code.
///
/// # Safety
///
/// The implementation must adhere exactly to the documentation
pub unsafe trait System: Sized {
    type Addrspace: Addrspace + Debug;
    type PageTable: Debug + Default;
    type ExecState: ExecState + Clone;
    type ArchCaps: Debug + Clone + ArchCaps<Self>;

    /// Initializes the architecture-specific subsystem
    fn init() -> Self;

    /// Returns a valid pointer to the currently active address space
    fn addrspace(&self) -> Self::Addrspace;

    /// Sets the per-cpu address of a core-local structure.
    fn set_core_data(&self, addr: VirtAddr);
}

pub trait ArchCaps<S: System> {
    fn new_addrspace<A>(addrspace: A) -> Self
    where
        A: Borrow<S::Addrspace>;

    fn as_addrspace(&self) -> Result<&KPtr<S::PageTable>, CapError>;
}
