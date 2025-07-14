use core::{marker::PhantomData, mem::MaybeUninit};

use derive_more::Debug;

#[cfg(feature = "userspace")]
pub mod ulib {
    use crate::caps::{CapError, CapResult, PositiveIsize};

    use super::SyscallArgs;
    use core::{arch::naked_asm, mem::MaybeUninit};

    #[unsafe(naked)]
    pub extern "C" fn syscall_raw(
        cap: usize,
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
    pub fn syscall<S>(args: SyscallArgs<S>) -> Result<PositiveIsize, CapError> {
        CapResult::from_isize(syscall_raw(
            args.cap,
            args.args[0],
            args.args[1],
            args.args[2],
            args.args[3],
            args.args[4],
        ))
    }
}

pub struct SyscallArgsInit;
pub struct SyscallArgsUninit;

#[derive(Debug)]
#[repr(C)]
pub struct SyscallArgs<InitStatus> {
    cap: usize,
    args: [MaybeUninit<usize>; 5],
    _phantom: PhantomData<InitStatus>,
}

impl<InitStatus> Copy for SyscallArgs<InitStatus> {}
impl<InitStatus> Clone for SyscallArgs<InitStatus> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<InitStatus> SyscallArgs<InitStatus> {
    /// Returns the capability ID to exercise with the syscall
    pub fn cap(&self) -> usize {
        self.cap
    }
}

impl SyscallArgs<SyscallArgsUninit> {
    /// Constructs a (possibly) partially uninitialized set of syscall arguments
    pub fn new_partial(cap: usize, args: [MaybeUninit<usize>; 5]) -> Self {
        Self {
            cap,
            args,
            _phantom: PhantomData,
        }
    }

    /// Converts the args into an initialized version of it.
    ///
    /// # Safety
    ///
    /// Caller must guarantee that the arguments have been initialized (at the type level).
    pub unsafe fn assume_args_init(self) -> SyscallArgs<SyscallArgsInit> {
        // SAFETY: Caller guarantess arguments are initialized
        unsafe { core::mem::transmute(self) }
    }

    /// Returns the arguments in this syscall
    pub fn args(&self) -> &[MaybeUninit<usize>; 5] {
        &self.args
    }

    /// Returns an uninitialized set of syscall arguments.
    pub fn uninit(cap: usize) -> Self {
        Self::new_partial(cap, [MaybeUninit::uninit(); 5])
    }
}

impl SyscallArgs<SyscallArgsInit> {
    pub fn new(cap: usize, args: [usize; 5]) -> Self {
        Self {
            cap,
            // SAFETY: MaybeUninit uses repr transparent.
            args: unsafe { core::mem::transmute::<[usize; 5], [MaybeUninit<usize>; 5]>(args) },
            _phantom: PhantomData,
        }
    }
    pub fn args(&self) -> &[usize; 5] {
        // SAFETY: Type state guarantees this is valid.
        unsafe { core::mem::transmute(&self.args) }
    }
}
