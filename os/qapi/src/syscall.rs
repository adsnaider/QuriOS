use core::{marker::PhantomData, mem::MaybeUninit};

use derive_more::{Debug, From, Into, TryFrom};

use crate::caps::CapError;

#[cfg(feature = "userspace")]
pub mod ulib {
    use crate::caps::{CapError, CapId, CapResult, PositiveIsize};

    use super::{SyscallOp, SyscallStruct};
    use core::{arch::naked_asm, mem::MaybeUninit};

    #[unsafe(naked)]
    pub extern "C" fn syscall_raw(
        cap: usize,
        op: SyscallOp,
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
    pub fn syscall<S: SyscallStruct>(cap: CapId, args: S) -> Result<PositiveIsize, CapError> {
        let args = args.into_args();
        CapResult::from_isize(syscall_raw(
            cap.into(),
            args.op,
            args.args[0],
            args.args[1],
            args.args[2],
            args.args[3],
        ))
    }
}

pub struct SyscallArgsInit;
pub struct SyscallArgsUninit;

pub type SyscallParams = [MaybeUninit<usize>; 4];

#[derive(Debug)]
#[repr(C)]
pub struct SyscallArgs<InitStatus> {
    op: SyscallOp,
    args: [MaybeUninit<usize>; 4],
    _phantom: PhantomData<InitStatus>,
}

impl<InitStatus> Copy for SyscallArgs<InitStatus> {}
impl<InitStatus> Clone for SyscallArgs<InitStatus> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<InitStatus> SyscallArgs<InitStatus> {
    pub fn into_uninit(self) -> SyscallArgs<SyscallArgsUninit> {
        SyscallArgs {
            op: self.op,
            args: self.args,
            _phantom: PhantomData,
        }
    }

    pub fn op(&self) -> SyscallOp {
        self.op
    }
}

impl SyscallArgs<SyscallArgsUninit> {
    /// Constructs a (possibly) partially uninitialized set of syscall arguments
    pub const fn new_uninit(op: SyscallOp) -> Self {
        Self {
            op,
            args: [const { MaybeUninit::uninit() }; 4],
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
    pub fn args(&self) -> &[MaybeUninit<usize>; 4] {
        &self.args
    }

    /// Returns the arguments in this syscall
    pub fn args_mut(&mut self) -> &mut [MaybeUninit<usize>; 4] {
        &mut self.args
    }
}

impl SyscallArgs<SyscallArgsInit> {
    pub fn new(op: SyscallOp, args: [usize; 4]) -> Self {
        Self {
            op,
            // SAFETY: MaybeUninit uses repr transparent.
            args: unsafe { core::mem::transmute::<[usize; 4], [MaybeUninit<usize>; 4]>(args) },
            _phantom: PhantomData,
        }
    }

    pub fn args(&self) -> &[usize; 4] {
        // SAFETY: Type state guarantees this is valid.
        unsafe { core::mem::transmute(&self.args) }
    }
}

pub trait SyscallStruct: Copy {
    fn into_args(self) -> SyscallArgs<SyscallArgsUninit>;
    fn try_from_args(args: SyscallArgs<SyscallArgsInit>) -> Result<Self, CapError>;
}

impl<T> SyscallStruct for SyscallArgs<T> {
    fn into_args(self) -> SyscallArgs<SyscallArgsUninit> {
        self.into_uninit()
    }

    fn try_from_args(args: SyscallArgs<SyscallArgsInit>) -> Result<Self, CapError> {
        // SAFETY: repr(C) with PhantomData guarantees same layout and size and struct validity.
        Ok(unsafe { core::mem::transmute::<SyscallArgs<SyscallArgsInit>, Self>(args) })
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, From, Into)]
pub struct SyscallOp(usize);
impl SyscallOp {
    pub const CAP_TABLE_CONS: Self = Self(0);

    pub const fn as_usize(self) -> usize {
        self.0
    }
}
