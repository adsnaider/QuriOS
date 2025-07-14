use arch::{ArchSystem, System};
use qapi::{
    caps::{CapError, CapIndex, PositiveIsize},
    syscall::{SyscallArgs, SyscallArgsInit},
};

use crate::thread::Thread;

pub fn syscall_handler(
    args: SyscallArgs<SyscallArgsInit>,
    ctx: <ArchSystem as System>::SyscallCtx,
) -> Result<PositiveIsize, CapError> {
    let cap = CapIndex::try_from(args.cap())?;
    let args = args.args();
    log::info!("Handling syscall: {cap} with args {args:?} {ctx:#?}");
    let _comp = Thread::<ArchSystem>::current()
        .unwrap()
        .active_comp()
        .cap(cap);
    Ok(10.try_into().unwrap())
}
