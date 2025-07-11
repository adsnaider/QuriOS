#![no_std]

use exec::ExecState;
use mem::{Addrspace, Frame, Pmo};
use qapi::syscall::{SyscallArgs, SyscallArgsInit};
use sync::cell::AtomicOnceCell;

#[cfg(target_arch = "x86_64")]
mod x86_64_impl;

pub mod exec;
pub mod mem;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        type ArchSystem = x86_64_impl::Sys;
    } else {
        const _: () = const { panic!("Target architecture not supported") };
    }
}

type SyscallHandler<S> = fn(SyscallArgs<SyscallArgsInit>, S) -> usize;
static SYSTEM: AtomicOnceCell<ArchSystem> = AtomicOnceCell::new();
pub fn system() -> &'static impl System {
    SYSTEM
        .get()
        .expect("System not fully initialized. Did you call `init`?")
}

/// Initializes the architecture-specific subsystem.
pub fn init(pmo: Pmo, syscall_handler: SyscallHandler<<ArchSystem as System>::SyscallCtx>) {
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
    fn init(pmo: Pmo, syscall_handler: SyscallHandler<Self::SyscallCtx>) -> Self;

    /// Returns a valid pointer to the currently active address space
    fn addrspace(&self) -> Self::Addrspace;
}

pub trait KernelObject: Sized {
    fn into_frame(self) -> Frame;
}

pub trait SyscallCtx: core::fmt::Debug {}
