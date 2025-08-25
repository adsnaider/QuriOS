#[cfg(target_arch = "x86_64")]
mod x86_64_impl;

pub mod mem;

use core::borrow::Borrow;
use core::fmt::Debug;

use mem::{Addrspace, PageFlags, VirtAddr};
use qapi::{
    caps::{CapError, PositiveIsize},
    init::{BootArgs, EntryFn},
    syscall::ops::{
        ctable::VMTableCons, introspect::IntrospectResult, vmtable::PaddedPageTableOffset,
    },
};
use sync::cell::AtomicOnceCell;

use crate::{kmem::KPtr, syscall::SyscallResp};

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
    type ExecState: ExecState<Sys = Self> + Clone;
    type ArchCaps: Debug + Clone + ArchCaps<Self>;
    type ExceptionAbi: Debug + Clone + InvokeAbi<Self>;
    type IrqCtx: Debug;
    type RetAbi: Debug + Default + RetAbi<Self>;

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

pub trait ExecState: Debug {
    type Sys: System;

    fn new_thread(entry: usize, stack_top: usize, arg0: usize) -> Self;
    fn for_init_comp(entry_fun: EntryFn, stack_top: *const (), arg0: *const BootArgs) -> Self;
    fn save(&self, ctx: &<Self::Sys as System>::IrqCtx);
    fn dispatch(&self) -> !;
}

pub trait InvokeAbi<Sys: System> {
    fn new_invocation(self, entry: usize, ctx: &Sys::IrqCtx) -> (Sys::ExecState, Sys::RetAbi);
}

pub trait RetAbi<Sys: System> {
    fn ret(self, callee_ctx: &Sys::IrqCtx, caller_ctx: &Sys::ExecState);
}
