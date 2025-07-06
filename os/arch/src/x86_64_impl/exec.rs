//! x86-64 execution context.
#![allow(unused)]

use core::{arch::naked_asm, mem::MaybeUninit};

use crate::SysCtx;

use super::gdt;

/// Preserved state from a syscall
pub struct SyscallCtx {
    control_regs: ControlRegs,
    preserved_regs: PreservedRegs,
}

impl SyscallCtx {
    /// Reads the syscall context from the stack
    ///
    /// # Safety
    ///
    /// Must be currently handling a syscall
    pub unsafe fn current() -> Self {
        let stack_end: *mut u64 = gdt::interrupt_stack_end().as_mut_ptr();
        let mut preserved: MaybeUninit<PreservedRegs> = MaybeUninit::uninit();
        // SAFETY: The precondition guarantees that preserved registers will be pushed to the stack.
        unsafe {
            core::ptr::copy_nonoverlapping(
                stack_end.sub(11) as *const PreservedRegs,
                preserved.as_mut_ptr(),
                1,
            );
        }
        Self {
            // SAFETY: We are currently executing a syscall by precondition
            control_regs: unsafe { Self::current_control() },
            // SAFETY: We are currently executing a syscall by precondition
            preserved_regs: unsafe { preserved.assume_init() },
        }
    }

    pub unsafe fn current_control() -> ControlRegs {
        let (rsp, rflags, rip);
        // SAFETY: We are currently executing a syscall by precondition
        unsafe {
            let stack_end: *mut u64 = gdt::interrupt_stack_end().as_mut_ptr();
            rsp = *stack_end.sub(2);
            rflags = *stack_end.sub(3);
            rip = *stack_end.sub(5);
        }
        ControlRegs { rflags, rsp, rip }
    }

    /// Updates the rflags register on this syscall.
    ///
    /// Note that this will update the rflags iff the syscall returns normally (i.e. no
    /// thread dispatching).
    ///
    /// # Safety
    ///
    /// Must be currently handling a syscall.
    pub unsafe fn update_flags(flags: u64) {
        let stack_end: *mut u64 = gdt::interrupt_stack_end().as_mut_ptr();
        // SAFETY: We are currently executing a syscall by precondition
        unsafe {
            *stack_end.sub(3) = flags;
        }
    }

    /// Gets the rflags registers (as it was before the syscall).
    ///
    /// # Safety
    ///
    /// Must be currently handling a syscall.
    pub unsafe fn get_flags() -> u64 {
        let stack_end: *mut u64 = gdt::interrupt_stack_end().as_mut_ptr();
        // SAFETY: We are currently executing a syscall by precondition
        unsafe { *stack_end.sub(3) }
    }
}

impl SysCtx for SyscallCtx {}

/// Execution context that can be dispatched.
#[repr(C)]
pub struct ExecCtx {
    regs: Regs,
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
    pub extern "sysv64" fn dispatch(&self) -> ! {
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
