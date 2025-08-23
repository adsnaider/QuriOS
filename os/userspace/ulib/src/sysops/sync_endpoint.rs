#[doc(hidden)]
pub use paste::paste;

#[macro_export]
macro_rules! make_sync_call {
    ($name:ident, $fun:path, $stacks:path $(,$attrs:meta)* $(,)?) => {
        $crate::sysops::sync_endpoint::paste! {
            extern "C" fn [<__ $name _inner>](a: usize, b: usize, c: usize, d: usize) -> qapi::caps::PositiveIsize
            {
                $fun(a, b, c, d)
            }
            #[unsafe(naked)]
            $(
                #[$attrs]
            )*
            pub extern "C" fn $name(a: usize, b: usize, c: usize, d: usize) -> qapi::caps::PositiveIsize
            {

                use stack_list::{stack_list_pop, stack_list_push};
                use core::arch::naked_asm;

                use qapi::{
                    syscall::{SyscallOp, ops::sync_ipc::SyncCallFun},
                };
                #[allow(unused_unsafe)]
                unsafe {
                    naked_asm!(
                        "mov r12, rdi",
                        "mov r13, rsi",
                        "mov r14, rdx",
                        "mov r15, rcx",
                        "lea rdi, [{stack_list}]",
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
                        "lea rdi, [{stack_list}]",
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
                        inner = sym [<__ $name _inner>],
                        stack_list = sym $stacks,
                        sync_ret_call = const { SyscallOp::SyncRet as usize },
                    )
                }
            }

            #[used]
            static [<HANDLER_ $name:upper _IS_USED>]: extern "C" fn(usize, usize, usize, usize)  -> ::qapi::caps::PositiveIsize = $name;
        }
    }
}
