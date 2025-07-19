//! x86-64 execution context.
#![allow(unused)]

use core::{arch::naked_asm, convert::Infallible, marker::PhantomData, mem::MaybeUninit};

use derive_more::Debug;

use qapi::init::{BootArgs, EntryFn};
use sealed::sealed;
use x86_64::{
    registers::rflags::RFlags,
    structures::idt::{
        DivergingHandlerFunc, DivergingHandlerFuncWithErrCode, Entry, HandlerFunc,
        HandlerFuncWithErrCode, InterruptStackFrame, InterruptStackFrameValue,
        PageFaultHandlerFunc,
    },
};

use crate::{SyscallCtx, exec::ExecState};

use super::gdt;

pub struct Exception;
pub struct Interrupt;

#[repr(transparent)]
#[derive(Debug)]
#[debug("{:?}", self.interrupt_stack_frame())]
pub struct ExceptionCtx<Kind> {
    stack_top: u64,
    _kind: PhantomData<Kind>,
}

impl SyscallCtx for ExceptionCtx<Interrupt> {}

impl<Kind> ExceptionCtx<Kind> {
    /// # Safety
    ///
    /// The stack_top must refer to the RSP register after the
    /// InterruptStackFrame is loaded (but before any error code or registers)
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
}

impl ExceptionCtx<Exception> {
    pub fn preserved_regs(&self) -> PreservedRegs {
        todo!();
    }

    pub fn scratch_regs(&self) -> ScratchRegs {
        todo!();
    }

    pub fn current_control(&self) -> ControlRegs {
        todo!();
    }

    pub fn error_code(&self) -> u64 {
        // SAFETY: Stack must contain error code below the interrupt stack frame
        unsafe { core::ptr::read((self.stack_top - 8) as *const u64) }
    }
}

impl ExceptionCtx<Interrupt> {
    pub fn preserved_regs(&self) -> PreservedRegs {
        todo!();
    }

    pub fn scratch_regs(&self) -> ScratchRegs {
        todo!();
    }

    pub fn current_control(&self) -> ControlRegs {
        todo!();
    }
}

/// Execution context that can be dispatched.
#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct ExecCtx {
    regs: Regs,
}

impl ExecState for ExecCtx {
    fn save(&self) {
        todo!()
    }

    fn dispatch(&self) -> ! {
        self.dispatch_raw();
    }

    fn for_entry(entry_fun: EntryFn, stack_top: *const (), arg0: *const BootArgs) -> Self {
        let mut regs = Regs::default();
        regs.scratch.rdi = arg0 as u64;
        regs.control.rip = entry_fun as *const () as u64;
        regs.control.rsp = stack_top as u64;
        // TODO: Maybe don't give access to all hardware here but it's good for debugging.
        regs.control.rflags =
            (RFlags::INTERRUPT_FLAG | RFlags::IOPL_HIGH | RFlags::IOPL_LOW).bits();
        Self { regs }
    }
}

impl ExecCtx {
    pub fn new(regs: Regs) -> Self {
        Self { regs }
    }

    pub fn regs(&self) -> &Regs {
        &self.regs
    }

    pub fn regs_mut(&mut self) -> &mut Regs {
        &mut self.regs
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
                "mov fs, ax",
                "mov gs, ax",
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
