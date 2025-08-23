pub mod ops;

use core::marker::PhantomData;
use core::mem::MaybeUninit;

use derive_more::{Debug, TryFrom};

use crate::caps::CapError;

pub struct SyscallArgsInit;
pub struct SyscallArgsUninit;

pub const SYSCALL_ARGS: usize = 5;
pub type UninitSyscallParams = [MaybeUninit<usize>; SYSCALL_ARGS];
pub type InitSyscallParams = [usize; SYSCALL_ARGS];

pub trait SyscallRequest: Sized {
    fn into_args(self) -> UninitSyscallParams;
    fn try_from_args(args: &InitSyscallParams) -> Result<Self, CapError>;
}

#[derive(Debug)]
#[repr(C)]
pub struct SyscallArgs<InitStatus> {
    op: usize,
    args: UninitSyscallParams,
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

    pub fn op(&self) -> Result<SyscallOp, CapError> {
        self.op.try_into().map_err(|_| CapError::InvalidOp)
    }

    pub fn raw_op(&self) -> usize {
        self.op
    }
}

impl SyscallArgs<SyscallArgsUninit> {
    /// Constructs a (possibly) partially uninitialized set of syscall arguments
    pub const fn new_uninit(op: SyscallOp) -> Self {
        Self {
            op: op as usize,
            args: [const { MaybeUninit::uninit() }; 5],
            _phantom: PhantomData,
        }
    }

    pub const fn new_with_args(op: SyscallOp, args: UninitSyscallParams) -> Self {
        Self {
            op: op as usize,
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
    pub fn args(&self) -> &UninitSyscallParams {
        &self.args
    }

    /// Returns the arguments in this syscall
    pub fn args_mut(&mut self) -> &mut UninitSyscallParams {
        &mut self.args
    }
}

impl SyscallArgs<SyscallArgsInit> {
    pub fn new(op: SyscallOp, args: InitSyscallParams) -> Self {
        Self {
            op: op as usize,
            // SAFETY: MaybeUninit uses repr transparent.
            args: unsafe { core::mem::transmute::<InitSyscallParams, UninitSyscallParams>(args) },
            _phantom: PhantomData,
        }
    }

    pub fn new_raw(op: usize, args: InitSyscallParams) -> Self {
        Self {
            op,
            // SAFETY: MaybeUninit uses repr transparent.
            args: unsafe { core::mem::transmute::<InitSyscallParams, UninitSyscallParams>(args) },
            _phantom: PhantomData,
        }
    }

    pub fn args(&self) -> &InitSyscallParams {
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

#[repr(usize)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, TryFrom)]
#[try_from(repr)]
#[non_exhaustive]
pub enum SyscallOp {
    CapCons = 1,
    CapDrop,
    CapCopy,
    CapLink,
    Retype,
    ThreadDispatch,
    VMLink,
    VMUnlink,
    VMSetAttr,
    SyncInvoke,
    SyncRet,
    SyncRetException,
    Introspect,
}
