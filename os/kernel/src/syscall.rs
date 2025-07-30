use crate::arch::{ArchSystem, System};
use cap_table::cap_table_cons;
use qapi::{
    caps::{cap_table::ConsOp, CapError, PositiveIsize},
    mem::RetypeOp,
    syscall::{SyscallArgs, SyscallArgsInit, SyscallOp},
};
use retyping::retype;

mod cap_table;
mod retyping;

pub fn syscall_handler(
    args: SyscallArgs<SyscallArgsInit>,
    ctx: <ArchSystem as System>::SyscallCtx,
) -> Result<PositiveIsize, CapError> {
    // SAFETY: We are allowed to get a mutable reference at the start of the syscall. It will get dropped
    log::debug!("Handling syscall: {args:?} {ctx:#?}");

    let op = args.op()?;
    match op {
        SyscallOp::CapTableCons => cap_table_cons(ConsOp::try_from_args(args.args())?),
        SyscallOp::Retype => retype(RetypeOp::try_from_args(args.args())?),
        _ => Err(CapError::SyscallNotImplemented),
    }
}
