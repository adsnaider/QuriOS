#![no_std]

use exec::ExecState;
use mem::{Addrspace, Frame, Pmo};
use sync::cell::AtomicOnceCell;

#[cfg(target_arch = "x86_64")]
mod x86_64_impl;

pub mod exec;
pub mod mem;

static SYSTEM: AtomicOnceCell<ArchSystem> = AtomicOnceCell::new();
pub fn system() -> &'static impl System {
    SYSTEM
        .get()
        .expect("System not fully initialized. Did you call `init`?")
}

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        type ArchSystem = x86_64_impl::Sys;
        /// Initializes the architecture-specific subsystem.
        pub fn init(pmo: Pmo) {
            SYSTEM.set(x86_64_impl::Sys::init(pmo)).expect("Tried to initialize system twice")
        }
    } else {
        const _: () = const { panic!("Target architecture not supported") };
    }
}

/// Architecture-agnostic system management
///
/// This trait serves to provide the general interface between the kernel
/// and the architecture-specific code.
///
/// # Safety
///
/// The implementation must adhere exactly to the documentation
pub unsafe trait System {
    type SysAddrspace: Addrspace;
    type SysExec: ExecState;

    /// Returns a valid pointer to the currently active address space
    fn addrspace(&self) -> Self::SysAddrspace;
}

pub trait KernelObject: Sized {
    fn into_frame(self) -> Frame;
}
