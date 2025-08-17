use qapi::{
    caps::{CapError, CapResult, PositiveIsize},
    syscall::SyscallStruct,
};

use core::{arch::naked_asm, mem::MaybeUninit};

#[unsafe(naked)]
pub extern "C" fn syscall_raw(
    op: usize,
    a: MaybeUninit<usize>,
    b: MaybeUninit<usize>,
    c: MaybeUninit<usize>,
    d: MaybeUninit<usize>,
    e: MaybeUninit<usize>,
) -> isize {
    // SAFETY: syscall expects arguments as an extern "C" call
    #[allow(unused_unsafe)]
    unsafe {
        naked_asm!("int 0x80", "ret")
    }
}

#[inline(always)]
pub fn syscall<S: SyscallStruct>(args: S) -> Result<PositiveIsize, CapError> {
    let args = args.into_args();
    CapResult::from_isize(syscall_raw(
        args.raw_op(),
        args.args()[0],
        args.args()[1],
        args.args()[2],
        args.args()[3],
        args.args()[4],
    ))
}
