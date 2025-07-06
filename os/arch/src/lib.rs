#![no_std]

use sync::cell::AtomicOnceCell;

#[cfg(target_arch = "x86_64")]
mod x86_64_impl;

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
        pub fn init() {
            SYSTEM.set(x86_64_impl::Sys::init()).expect("Tried to initialize system twice")
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
    /// Returns the context around a syscall
    ///
    /// # Safety
    ///
    /// Kernel must be currently executing a syscall.
    unsafe fn syscall_ctx(&self) -> impl SysCtx;
}

pub trait SysCtx: Sized {}
