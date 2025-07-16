#![no_std]

use core::fmt::Debug;

use exec::ExecState;
use mem::{Addrspace, Frame, Pmo};
use qapi::{
    caps::{CapError, CapabilityKind, PositiveIsize},
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
    type Addrspace: Addrspace + Debug;
    type PageTable: Debug + Default + CapabilityResource;
    type ExecState: ExecState + Clone;
    type SyscallCtx: SyscallCtx + Debug;

    /// Initializes the architecture-specific subsystem
    fn init(pmo: Pmo, syscall_handler: SyscallHandler<Self>) -> Self;

    /// Returns a valid pointer to the currently active address space
    fn addrspace(&self) -> Self::Addrspace;
}

pub trait SyscallCtx {}

pub trait CapabilityResource {
    fn exercise(&self, args: &[usize; 5], kind: CapabilityKind) -> Result<PositiveIsize, CapError>;
}
