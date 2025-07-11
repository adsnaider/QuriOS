use arch::SyscallCtx;
use qapi::syscall::{SyscallArgs, SyscallArgsInit};

pub fn syscall_handler(args: SyscallArgs<SyscallArgsInit>, ctx: impl SyscallCtx) -> usize {
    let cap = args.cap();
    let args = args.args();
    log::info!("Handling syscall: {cap} with args {args:?} {ctx:#?}");
    todo!();
}
