use core::arch::naked_asm;

#[doc(hidden)]
pub use paste::paste;
use qapi::{
    caps::{
        PositiveIsize,
        sync_ipc::{ExceptionAbi, StandardAbi, SyncAbi},
    },
    syscall::SyscallOp,
};
use stack_list::{StackList, stack_list_pop, stack_list_push};

pub const IPC_STACK_BUCKETS: usize = 4;
pub static IPC_STACKS: [StackList<'static>; IPC_STACK_BUCKETS] =
    [const { StackList::new() }; IPC_STACK_BUCKETS];

pub struct SyncEndpoint<const BUCKET: usize, Abi, F>(Abi, F);

pub trait AbiImpl {
    extern "C" fn os_entry();
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
    extern "C" fn os_entry() {
        extern "C" fn routine<F>(a: usize, b: usize, c: usize, d: usize) -> PositiveIsize
        where
            F: AbiFn<StandardAbi>,
        {
            let fun: *const F = core::ptr::dangling();
            // SAFETY: Zero-sized type can alwasy be dereferenced as it's a compile-type only object.
            unsafe { (*fun)((a, b, c, d)) }
        }

        // SAFETY: Proper conditions for a synchronous invocation call gate
        #[allow(unused_unsafe)]
        unsafe {
            naked_asm!(
                "call {inner}",
                "mov rdi, {sync_ret_call}",
                "mov rsi, rax",
                "int 0x80",
                "ud2",
                inner = sym routine::<F>,
                sync_ret_call = const {SyscallOp::SyncRet as usize},
            );
        }
    }
}

impl<const BUCKET: usize, F> AbiImpl for SyncEndpoint<BUCKET, ExceptionAbi, F>
where
    F: AbiFn<ExceptionAbi>,
{
    #[unsafe(naked)]
    extern "C" fn os_entry() {
        extern "C" fn routine<F>(kind: usize, code: u64)
        where
            F: AbiFn<ExceptionAbi>,
        {
            let fun: *const F = core::ptr::dangling();
            // SAFETY: Zero-sized type can alwasy be dereferenced as it's a compile-type only object.
            unsafe { (*fun)((kind, code)) }
        }

        // SAFETY: Proper conditions for a synchronous invocation call gate
        #[allow(unused_unsafe)]
        unsafe {
            naked_asm!(
                "call {inner}",
                "mov rdi, {sync_ret_call}",
                "int 0x80",
                "ud2",
                inner = sym routine::<F>,
                sync_ret_call = const {SyscallOp::SyncRet as usize},
            );
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
        Self::os_entry
    }

    pub const fn stackful_endpoint(&self) -> extern "C" fn() {
        #[unsafe(link_section = ".exception_handler")]
        #[unsafe(naked)]
        extern "C" fn entry<This: AbiImpl, const BUCKET: usize>() {
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
                    inner = sym This::os_entry,
                    stack_list = sym IPC_STACKS,
                    stack_bucket_off = const { BUCKET * size_of::<StackList>() },
                    sync_ret_call = const { SyscallOp::SyncRet as usize },
                )
            }
        }
        entry::<Self, BUCKET>
    }
}
