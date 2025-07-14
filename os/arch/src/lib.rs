#![no_std]

use exec::ExecState;
use mem::{Addrspace, Frame, Pmo};
use qapi::{
    caps::{CapError, PositiveIsize},
    syscall::{SyscallArgs, SyscallArgsInit},
};
use sync::cell::AtomicOnceCell;

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

type SyscallHandler<S> =
    fn(SyscallArgs<SyscallArgsInit>, <S as System>::SyscallCtx) -> Result<PositiveIsize, CapError>;

static SYSTEM: AtomicOnceCell<ArchSystem> = AtomicOnceCell::new();
pub fn system() -> &'static ArchSystem {
    SYSTEM
        .get()
        .expect("System not fully initialized. Did you call `init`?")
}

/// Initializes the architecture-specific subsystem.
pub fn init(pmo: Pmo, syscall_handler: SyscallHandler<ArchSystem>) {
    SYSTEM
        .set(ArchSystem::init(pmo, syscall_handler))
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
    type Addrspace: Addrspace;
    type ExecState: ExecState;
    type SyscallCtx: SyscallCtx;

    /// Initializes the architecture-specific subsystem
    fn init(pmo: Pmo, syscall_handler: SyscallHandler<Self>) -> Self;

    /// Returns a valid pointer to the currently active address space
    fn addrspace(&self) -> Self::Addrspace;
}

pub trait KernelObject: Sized {
    type System: System;

    /// Transforms this object into a raw frame (but maintains any form of
    /// refernce counting)
    fn into_frame(self) -> Frame;

    /// Turns the frame into the kernel object (maintaining any form of reference
    /// counting)
    ///
    /// # Safety
    ///
    /// The frame must have been created with `into_frame` and frame must still contain
    /// the original object (though possibly mutated).
    unsafe fn from_frame(sys: &Self::System, frame: Frame) -> Self;
}

pub trait SyscallCtx: core::fmt::Debug {}
