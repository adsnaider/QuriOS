#[cfg(target_arch = "x86_64")]
mod x86_64_impl;
#[cfg(target_arch = "x86_64")]
pub use x86_64_impl::backtrace;

pub mod mem;

use core::borrow::Borrow;
use core::fmt::Debug;

use mem::{Addrspace, PageFlags, VirtAddr};
use qapi::caps::CapError;
use qapi::caps::sync_ipc::SyncAbi;
use qapi::init::{BootArgs, EntryFn};
use qapi::syscall::ops::ctable::VMTableCons;
use qapi::syscall::ops::introspect::IntrospectResult;
use qapi::syscall::ops::vmtable::PaddedPageTableOffset;
use sync::cell::AtomicOnceCell;

use crate::kmem::KPtr;
use crate::notify::Notification;
use crate::syscall::SyscallResp;

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
    type Addrspace: Addrspace + Debug + Clone;
    type ExecState: ExecState<Sys = Self> + Clone;
    type ArchCaps: ArchCaps<Self> + Debug + Clone;
    type ExceptionAbi: InvokeAbi<Self> + Debug + Clone;
    type PageTable: Debug + Default;
    type IrqCtx: Debug;

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
    fn irq_ctrl() -> Result<Self, CapError>;
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
    fn irq_set(&self, irq: usize, notification: Option<Notification<S>>) -> SyscallResp;
    fn irq_unset(&self, irq: usize) -> SyscallResp {
        self.irq_set(irq, None)
    }
}

pub trait ExecState: Debug {
    type Sys: System;

    fn new_thread(entry: usize, stack_top: usize, arg0: usize) -> Self;
    fn for_init_comp(entry_fun: EntryFn, stack_top: *const (), arg0: *const BootArgs) -> Self;
    fn save(&self, ctx: &<Self::Sys as System>::IrqCtx);
    fn dispatch(&self) -> !;
    fn set_out_reg(&self, value: usize);
}

pub trait InvokeAbi<Sys: System>: SyncAbi {
    fn invoke_with_args(&self, args: Self::Args, entry: usize) -> Sys::ExecState;
    fn ret_with_args(&self, ret: Self::Ret, caller_ctx: &Sys::ExecState);
    fn invoke_passthrough(&self, ctx: &Sys::IrqCtx, entry: usize) -> Sys::ExecState;
    fn ret_passthrough(&self, callee_ctx: &Sys::IrqCtx, caller_ctx: &Sys::ExecState);
}
