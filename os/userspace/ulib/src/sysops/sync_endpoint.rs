use core::arch::naked_asm;

#[doc(hidden)]
pub use paste::paste;
use qapi::{
    caps::{
        PositiveIsize,
        sync_ipc::{ExceptionAbi, ExceptionArgs, StandardAbi, SyncAbi},
    },
    syscall::SyscallOp,
};
use stack_list::{StackList, stack_list_pop, stack_list_push};

pub const IPC_STACK_BUCKETS: usize = 4;
pub static IPC_STACKS: [StackList<'static>; IPC_STACK_BUCKETS] =
    [const { StackList::new() }; IPC_STACK_BUCKETS];

#[derive(Debug, Copy, Clone)]
pub struct SyncEndpoint<const BUCKET: usize, Abi, F>(Abi, F);

pub trait AbiImpl {
    extern "C" fn stackless_entry();
    extern "C" fn stackfull_entry();
}

trait AbiFn<Abi: SyncAbi>: Fn(<Abi as SyncAbi>::Args) -> <Abi as SyncAbi>::Ret {}
impl<F, Abi> AbiFn<Abi> for F
where
    F: Fn(<Abi as SyncAbi>::Args) -> <Abi as SyncAbi>::Ret,
    Abi: SyncAbi,
{
}

impl<const BUCKET: usize, F> AbiImpl for SyncEndpoint<BUCKET, StandardAbi, F>
where
    F: AbiFn<StandardAbi>,
{
    #[unsafe(naked)]
    extern "C" fn stackless_entry() {
        // SAFETY: Proper conditions for a synchronous invocation call gate
        #[allow(unused_unsafe)]
        unsafe {
            naked_asm!(
                "call {inner}",
                "mov rdi, {sync_ret_call}",
                "mov rsi, rax",
                "int 0x80",
                "ud2",
                inner = sym Self::routine,
                sync_ret_call = const { SyscallOp::SyncRet as usize },
            );
        }
    }

    #[unsafe(naked)]
    extern "C" fn stackfull_entry() {
        #[allow(unused_unsafe)]
        unsafe {
            naked_asm!(
                "mov r12, rdi",
                "mov r13, rsi",
                "mov r14, rdx",
                "mov r15, rcx",
                "lea rdi, [{stack_list} + {stack_bucket_off}]",
                stack_list_pop!(),
                "test rax, rax",
                "je 3f",
                "mov rsp, rax",
                "mov rdi, r12",
                "mov rsi, r13",
                "mov rdx, r14",
                "mov rcx, r15",
                "call {inner}",
                "mov r12, rax",
                "lea rdi, [{stack_list} + {stack_bucket_off}]",
                "mov rsi, rsp",
                stack_list_push!(),
                "mov rax, r12",
                "jmp 4f",
                "3:",
                 "mov rax, 1",
                "4:",
                 "mov rsp, 0",
                 "mov rdi, {sync_ret_call}",
                 "mov rsi, rax",
                "int 0x80",
                "ud2",
                inner = sym Self::routine,
                stack_list = sym IPC_STACKS,
                stack_bucket_off = const { BUCKET * size_of::<StackList>() },
                sync_ret_call = const { SyscallOp::SyncRet as usize },
            )
        }
    }
}

impl<const BUCKET: usize, F> AbiImpl for SyncEndpoint<BUCKET, ExceptionAbi, F>
where
    F: AbiFn<ExceptionAbi>,
{
    #[unsafe(naked)]
    extern "C" fn stackless_entry() {
        // SAFETY: Proper conditions for a synchronous invocation call gate
        #[allow(unused_unsafe)]
        unsafe {
            naked_asm!(
                "call {inner}",
                "mov rdi, {sync_ret_call}",
                "int 0x80",
                "ud2",
                inner = sym Self::routine,
                sync_ret_call = const {SyscallOp::SyncRet as usize},
            );
        }
    }

    #[unsafe(naked)]
    extern "C" fn stackfull_entry() {
        #[allow(unused_unsafe)]
        unsafe {
            naked_asm!(
                "mov r12, rdi",
                "mov r13, rsi",
                "mov r14, rdx",
                "lea rdi, [{stack_list} + {stack_bucket_off}]",
                stack_list_pop!(),
                "test rax, rax",
                "je 3f",
                "mov rsp, rax",
                "mov rdi, r12",
                "mov rsi, r13",
                "mov rdx, r14",
                "call {inner}",
                "mov r12, rax",
                "lea rdi, [{stack_list} + {stack_bucket_off}]",
                "mov rsi, rsp",
                stack_list_push!(),
                "mov rax, r12",
                "jmp 4f",
                "3:",
                 "mov rax, 1",
                "4:",
                 "mov rsp, 0",
                 "mov rdi, {sync_ret_call}",
                 "mov rsi, rax",
                "int 0x80",
                "ud2",
                inner = sym Self::routine,
                stack_list = sym IPC_STACKS,
                stack_bucket_off = const { BUCKET * size_of::<StackList>() },
                sync_ret_call = const { SyscallOp::SyncRet as usize },
            )
        }
    }
}

impl<const BUCKET: usize, Abi, F> SyncEndpoint<BUCKET, Abi, F>
where
    Self: AbiImpl,
{
    pub const fn create(abi: Abi, fun: F) -> Self {
        const {
            assert!(
                size_of::<F>() == 0,
                "Stackless invocations must be zero-sized closures"
            );
        }
        Self(abi, fun)
    }

    pub const fn endpoint(&self) -> extern "C" fn() {
        Self::stackless_entry
    }

    pub const fn stackfull_endpoint(&self) -> extern "C" fn() {
        Self::stackfull_entry
    }
}

impl<const BUCKET: usize, F> SyncEndpoint<BUCKET, ExceptionAbi, F>
where
    F: AbiFn<ExceptionAbi>,
{
    extern "C" fn routine(kind: usize, code: u64, extra: u64) {
        let fun: *const F = core::ptr::dangling();
        // SAFETY: Zero-sized type can alwasy be dereferenced as it's a compile-type only object.
        unsafe { (*fun)(ExceptionArgs { kind, code, extra }) }
    }
}

impl<const BUCKET: usize, F> SyncEndpoint<BUCKET, StandardAbi, F>
where
    F: AbiFn<StandardAbi>,
{
    extern "C" fn routine(a: usize, b: usize, c: usize, d: usize) -> PositiveIsize {
        let fun: *const F = core::ptr::dangling();
        // SAFETY: Zero-sized type can alwasy be dereferenced as it's a compile-type only object.
        unsafe { (*fun)((a, b, c, d)) }
    }
}
