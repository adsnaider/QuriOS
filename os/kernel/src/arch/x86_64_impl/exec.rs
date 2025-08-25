//! x86-64 execution context.
#![allow(unused)]

use core::arch::naked_asm;
use core::cell::Cell;
use core::fmt::{Octal, UpperHex};
use core::marker::PhantomData;
use core::mem::MaybeUninit;

use derive_more::{Debug, From};
use derive_where::derive_where;
use qapi::caps::PositiveIsize;
use qapi::exception::ExceptionKind;
use qapi::init::{BootArgs, EntryFn};
use sealed::sealed;
use x86_64::registers::rflags::RFlags;
use x86_64::structures::idt::{
    DivergingHandlerFunc, DivergingHandlerFuncWithErrCode, Entry, HandlerFunc,
    HandlerFuncWithErrCode, InterruptStackFrame, InterruptStackFrameValue, PageFaultHandlerFunc,
};

use super::{X64Sys, gdt};
use crate::arch::{ExecState, InvokeAbi, RetAbi, System};
use crate::sync_call::CallAbi;

pub struct Exception;
pub struct Interrupt;

#[derive(Debug)]
pub enum IrqCtx {
    Exception(ExceptionCtx<Exception>),
    Interrupt(ExceptionCtx<Interrupt>),
}

#[repr(transparent)]
#[derive_where(Clone, Copy)]
pub struct ExceptionCtx<Kind> {
    stack_top: u64,
    _kind: PhantomData<Kind>,
}
impl<Kind> core::fmt::Debug for ExceptionCtx<Kind> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ExceptionCtx")
            .field("stack_top", &format_args!("{:#X}", self.stack_top))
            .field("frame", self.interrupt_stack_frame())
            .finish()
    }
}

impl<Kind> ExceptionCtx<Kind> {
    /// # Safety
    ///
    /// The stack_top must refer to the RSP register after the
    /// InterruptStackFrame is loaded (but before any error code or registers)
    ///
    /// Only one ExceptionCtx may be instantiated per exception/interrupt
    pub unsafe fn new(stack_top: u64) -> Self {
        Self {
            stack_top,
            _kind: PhantomData,
        }
    }

    /// Returns the interrupt stack frame for this ISR
    pub fn interrupt_stack_frame(&self) -> &InterruptStackFrameValue {
        // SAFETY: We can assume the stack_top is valid as it was created with unsafe code to guarantee that.
        unsafe { &*(self.stack_top as usize as *const InterruptStackFrameValue) }
    }

    /// Returns the interrupt stack frame for this ISR
    ///
    /// # Safety
    ///
    /// Modifying the interrupt stack frame may result in undefined behavior in numerous ways
    pub unsafe fn interrupt_stack_frame_mut(&mut self) -> &mut InterruptStackFrameValue {
        // SAFETY: We can assume the stack_top is valid as it was created with unsafe code to guarantee that.
        unsafe { &mut *(self.stack_top as usize as *mut InterruptStackFrameValue) }
    }

    pub fn ctrl_regs(&self) -> ControlRegs {
        let isr = self.interrupt_stack_frame();
        ControlRegs {
            rflags: isr.cpu_flags.bits(),
            rsp: isr.stack_pointer.as_u64(),
            rip: isr.instruction_pointer.as_u64(),
        }
    }
}

impl ExceptionCtx<Interrupt> {
    pub fn preserved_regs(&self) -> &PreservedRegs {
        // SAFETY: Preserved regs are pushed after scratch registers. Should be safe to read this.
        unsafe {
            let preserved = (self.stack_top as usize
                - size_of::<PreservedRegs>()
                - size_of::<ScratchRegs>()) as *const PreservedRegs;
            &*preserved
        }
    }

    pub fn scratch_regs(&self) -> &ScratchRegs {
        let scratch = (self.stack_top as usize - size_of::<ScratchRegs>()) as *const ScratchRegs;
        // SAFETY: Scratch registers are pushed first before, so should be here.
        unsafe { &*scratch }
    }

    #[allow(clippy::mut_from_ref)]
    pub unsafe fn scratch_regs_mut(&self) -> &mut ScratchRegs {
        let scratch = (self.stack_top as usize - size_of::<ScratchRegs>()) as *mut ScratchRegs;
        // SAFETY: Scratch registers are pushed first before, so should be here.
        unsafe { &mut *scratch }
    }
}

impl ExceptionCtx<Exception> {
    pub fn error_code(&self) -> u64 {
        // SAFETY: Stack must contain error code below the interrupt stack frame
        unsafe { core::ptr::read((self.stack_top - 8) as *const u64) }
    }

    pub fn preserved_regs(&self) -> &PreservedRegs {
        // SAFETY: Preserved regs are pushed after scratch registers. Should be safe to read this.
        unsafe {
            let preserved = (self.stack_top as usize
                - size_of::<PreservedRegs>()
                - size_of::<ScratchRegs>()
                - 8) as *const PreservedRegs;
            &*preserved
        }
    }

    pub fn scratch_regs(&self) -> &ScratchRegs {
        let scratch =
            (self.stack_top as usize - 8 - size_of::<ScratchRegs>()) as *const ScratchRegs;
        // SAFETY: Scratch registers are pushed first before, so should be here.
        unsafe { &*scratch }
    }

    #[allow(clippy::mut_from_ref)]
    pub unsafe fn scratch_regs_mut(&self) -> &mut ScratchRegs {
        let scratch = (self.stack_top as usize - 8 - size_of::<ScratchRegs>()) as *mut ScratchRegs;
        // SAFETY: Scratch registers are pushed first before, so should be here.
        unsafe { &mut *scratch }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ExceptionAbi {
    which: ExceptionKind,
    code: Option<u64>,
}

impl ExceptionAbi {
    pub const fn new(which: ExceptionKind, code: Option<u64>) -> Self {
        Self { which, code }
    }
}

/// Execution context that can be dispatched.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct ExecCtx {
    regs: Cell<Regs>,
}

impl IrqCtx {
    /// Returns the interrupt stack frame for this ISR
    pub fn interrupt_stack_frame(&self) -> &InterruptStackFrameValue {
        match self {
            IrqCtx::Exception(ctx) => ctx.interrupt_stack_frame(),
            IrqCtx::Interrupt(ctx) => ctx.interrupt_stack_frame(),
        }
    }

    /// Returns the interrupt stack frame for this ISR
    ///
    /// # Safety
    ///
    /// Modifying the interrupt stack frame may result in undefined behavior in numerous ways
    pub unsafe fn interrupt_stack_frame_mut(&mut self) -> &mut InterruptStackFrameValue {
        match self {
            // SAFETY: Precondition
            IrqCtx::Exception(ctx) => unsafe { ctx.interrupt_stack_frame_mut() },
            // SAFETY: Precondition
            IrqCtx::Interrupt(ctx) => unsafe { ctx.interrupt_stack_frame_mut() },
        }
    }

    pub fn preserved_regs(&self) -> &PreservedRegs {
        match self {
            IrqCtx::Exception(ctx) => ctx.preserved_regs(),
            IrqCtx::Interrupt(ctx) => ctx.preserved_regs(),
        }
    }

    pub fn scratch_regs(&self) -> &ScratchRegs {
        match self {
            IrqCtx::Exception(ctx) => ctx.scratch_regs(),
            IrqCtx::Interrupt(ctx) => ctx.scratch_regs(),
        }
    }

    #[allow(clippy::mut_from_ref)]
    pub unsafe fn scratch_regs_mut(&self) -> &mut ScratchRegs {
        match self {
            // SAFETY: Precondition
            IrqCtx::Exception(ctx) => unsafe { ctx.scratch_regs_mut() },
            // SAFETY: Precondition
            IrqCtx::Interrupt(ctx) => unsafe { ctx.scratch_regs_mut() },
        }
    }

    pub fn ctrl_regs(&self) -> ControlRegs {
        match self {
            IrqCtx::Exception(ctx) => ctx.ctrl_regs(),
            IrqCtx::Interrupt(ctx) => ctx.ctrl_regs(),
        }
    }

    pub fn regs(&self) -> Regs {
        let preserved = self.preserved_regs();
        let scratch = self.scratch_regs();
        let control = self.ctrl_regs();
        Regs {
            scratch: *scratch,
            preserved: *preserved,
            control,
        }
    }

    pub fn error_code(&self) -> Option<u64> {
        match self {
            IrqCtx::Exception(ctx) => Some(ctx.error_code()),
            IrqCtx::Interrupt(ctx) => None,
        }
    }
}

impl ExecState for ExecCtx {
    type Sys = X64Sys;

    fn save(&self, ctx: &IrqCtx) {
        let regs = ctx.regs();
        self.regs.set(regs);
    }

    fn dispatch(&self) -> ! {
        self.dispatch_raw();
    }

    fn for_init_comp(entry_fun: EntryFn, stack_top: *const (), arg0: *const BootArgs) -> Self {
        let mut regs = Regs::default();
        regs.scratch.rdi = arg0 as u64;
        regs.control.rip = entry_fun as *const () as u64;
        regs.control.rsp = stack_top as u64;
        // TODO: Maybe don't give access to all hardware here but it's good for debugging.
        regs.control.rflags =
            (RFlags::INTERRUPT_FLAG | RFlags::IOPL_HIGH | RFlags::IOPL_LOW).bits();
        Self {
            regs: Cell::new(regs),
        }
    }

    fn new_thread(entry: usize, stack_top: usize, arg0: usize) -> Self {
        let mut regs = Regs::default();
        regs.control.rip = entry as u64;
        regs.control.rsp = stack_top as u64;
        regs.scratch.rdi = arg0 as u64;
        // TODO: Maybe don't give access to all hardware here but it's good for debugging.
        regs.control.rflags =
            (RFlags::INTERRUPT_FLAG | RFlags::IOPL_HIGH | RFlags::IOPL_LOW).bits();
        Self {
            regs: Cell::new(regs),
        }
    }
}

impl ExecCtx {
    pub fn new(regs: Regs) -> Self {
        Self {
            regs: Cell::new(regs),
        }
    }

    pub fn regs(&self) -> Regs {
        self.regs.get()
    }

    #[unsafe(naked)]
    extern "sysv64" fn dispatch_raw(&self) -> ! {
        // SAFETY: We are only jumping to userspace, guaranteeing address space separation, so it doesn't matter
        // what we are actually jumping to.
        #[allow(unused_unsafe)]
        unsafe {
            naked_asm!(
                "pop rax",
                // Setup the segment selectors
                "mov ax, (4 * 8) | 3",
                "mov ds, ax",
                "mov es, ax",
                // Restore SCRATCH
                "mov rax, [rdi + 8*0]",
                "mov rcx, [rdi + 8*1]",
                "mov rdx, [rdi + 8*2]",
                "mov rsi, [rdi + 8*3]",
                // RDI: Later as it holds arg0
                "mov r8, [rdi + 8*5]",
                "mov r9, [rdi + 8*6]",
                "mov r10, [rdi + 8*7]",
                "mov r11, [rdi + 8*8]",
                // Restore PRESEVED
                "mov rbx, [rdi + 8*9]",
                "mov rbp, [rdi + 8*10]",
                "mov r12, [rdi + 8*11]",
                "mov r13, [rdi + 8*12]",
                "mov r14, [rdi + 8*13]",
                "mov r15, [rdi + 8*14]",
                "push (4 * 8) | 3",     // SS
                "push [rdi + 8*16]",    // Push rsp
                "push [rdi + 8*15]",    // push rflags
                "push (3 * 8) | 3",     // CS with RPL 3
                "push [rdi + 8*17]",    // Push the new instruction pointer
                "mov rdi, [rdi + 8*4]", // And the RDI register
                "swapgs",
                "iretq",
            )
        }
    }
}

// SAFETY: Don't change the order of any of these
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct ScratchRegs {
    pub rax: u64, // Off: 0
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64, // Off: 5
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
}

// SAFETY: Don't change the order of any of these
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct PreservedRegs {
    pub rbx: u64,
    pub rbp: u64, // Off: 10
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

// SAFETY: Don't change the order of any of these
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct ControlRegs {
    pub rflags: u64, // Off: 15
    pub rsp: u64,
    pub rip: u64,
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct Regs {
    pub scratch: ScratchRegs,
    pub preserved: PreservedRegs,
    pub control: ControlRegs,
}

impl InvokeAbi<X64Sys> for CallAbi {
    fn new_invocation(self, entry: usize, ctx: &IrqCtx) -> (ExecCtx, AnyRet) {
        let curr_regs = ctx.regs();
        let mut regs = Regs::default();
        regs.control.rip = entry as u64;
        regs.control.rsp = 0;
        // NOTE: RDI and RSI contain the SyncCallOp and SyncCall cap respectively.
        regs.scratch.rdi = curr_regs.scratch.rdx;
        regs.scratch.rsi = curr_regs.scratch.rcx;
        regs.scratch.rdx = curr_regs.scratch.r8;
        regs.scratch.rcx = curr_regs.scratch.r9;
        // TODO: Maybe don't give access to all hardware here but it's good for debugging.
        regs.control.rflags =
            (RFlags::INTERRUPT_FLAG | RFlags::IOPL_HIGH | RFlags::IOPL_LOW).bits();
        (
            ExecCtx {
                regs: Cell::new(regs),
            },
            CallRetAbi.into(),
        )
    }
}

impl InvokeAbi<X64Sys> for ExceptionAbi {
    fn new_invocation(self, entry: usize, ctx: &IrqCtx) -> (ExecCtx, AnyRet) {
        let mut regs = Regs::default();
        regs.control.rip = entry as u64;
        regs.control.rsp = 0;
        regs.scratch.rdi = self.which as u64;
        regs.scratch.rsi = self.code.unwrap_or(u64::MAX);
        regs.control.rflags =
            (RFlags::INTERRUPT_FLAG | RFlags::IOPL_HIGH | RFlags::IOPL_LOW).bits();
        (
            ExecCtx {
                regs: Cell::new(regs),
            },
            ExceptionRetAbi.into(),
        )
    }
}

impl RetAbi<X64Sys> for CallRetAbi {
    fn ret(self, callee_ctx: &IrqCtx, caller_ctx: &ExecCtx) {
        let resp = caller_ctx.regs().scratch.rsi as isize;
        let resp = resp.max(0);
        caller_ctx.regs.update(|mut regs| {
            regs.scratch.rax = resp as u64;
            regs
        });
    }
}
impl RetAbi<X64Sys> for ExceptionRetAbi {
    fn ret(self, callee_ctx: &IrqCtx, caller_ctx: &ExecCtx) {
        // Purposely empty: Exceptions are essentially as transparent as they get. In the future,
        // we may have a way to return ways of udpating specific control registers (like to push the
        // RIP forward)
    }
}

#[derive(Debug)]
pub struct ExceptionRetAbi;
#[derive(Debug)]
pub struct CallRetAbi;

#[derive(Debug, Default, From)]
pub enum AnyRet {
    #[default]
    NoReturn,
    Standard(CallRetAbi),
    Exception(ExceptionRetAbi),
}

impl RetAbi<X64Sys> for AnyRet {
    fn ret(
        self,
        callee_ctx: &<X64Sys as System>::IrqCtx,
        caller_ctx: &<X64Sys as System>::ExecState,
    ) {
        match self {
            AnyRet::NoReturn => panic!("Attempted return on no-return ABI"),
            AnyRet::Standard(call_abi) => call_abi.ret(callee_ctx, caller_ctx),
            AnyRet::Exception(exception_ret_abi) => exception_ret_abi.ret(callee_ctx, caller_ctx),
        }
    }
}
