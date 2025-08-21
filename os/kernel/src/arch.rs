use core::borrow::Borrow;
use core::fmt::Debug;

use exec::ExecState;
use mem::{Addrspace, Frame, PageFlags, VirtAddr};
use qapi::{
    caps::CapError,
    syscall::ops::{
        ctable::VMTableCons, introspect::IntrospectResult, vmtable::PaddedPageTableOffset,
    },
};
use sync::cell::AtomicOnceCell;

use crate::{kmem::KPtr, syscall::SyscallResp};

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

    /// Perform any post-initialization steps that require further system boot.
    fn post_init(&self);

    /// Returns a valid pointer to the currently active address space
    fn addrspace(&self) -> Self::Addrspace;

    /// Sets the per-cpu address of a core-local structure.
    fn set_core_data(&self, addr: VirtAddr);
}

pub trait ArchCaps<S: System>: Sized {
    fn new_vmtable(args: VMTableCons) -> Result<Self, CapError>;
    fn new_addrspace<A>(addrspace: A) -> Self
    where
        A: Borrow<S::Addrspace>;

    fn as_addrspace(&self) -> Result<&KPtr<S::PageTable>, CapError>;
    fn as_vmtable(&self) -> Result<&KPtr<S::PageTable>, CapError>;
    fn vm_link(
        top_table: &Self,
        slot: PaddedPageTableOffset,
        bottom_table: &Self,
        page_flags: PageFlags,
    ) -> SyscallResp;
    fn vm_unlink(table: &Self, slot: PaddedPageTableOffset) -> SyscallResp;
    fn vm_set_attributes(
        table: &Self,
        slot: PaddedPageTableOffset,
        attributes: PageFlags,
    ) -> SyscallResp;

    fn introspect(&self) -> IntrospectResult;
}
