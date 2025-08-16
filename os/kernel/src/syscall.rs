use crate::arch::{exec::ExecState, ArchSystem, System};
use cap_table::cap_table_cons;
use qapi::{
    caps::{cap_table::ConsOp, thread::DispatchOp, CapError, PositiveIsize},
    mem::phys::RetypeOp,
    syscall::{SyscallArgs, SyscallArgsInit, SyscallOp},
};
use retyping::retype;
use thread::dispatch;

mod cap_table;
mod retyping;
mod thread;

pub fn syscall_handler(
    args: SyscallArgs<SyscallArgsInit>,
    ctx: <<ArchSystem as System>::ExecState as ExecState>::RegCtx,
) -> Result<PositiveIsize, CapError> {
    // SAFETY: We are allowed to get a mutable reference at the start of the syscall. It will get dropped
    log::trace!("Handling syscall: {args:?} {ctx:#?}");

    let op = args.op()?;
    match op {
        SyscallOp::CapCons => cap_table_cons(ConsOp::try_from_args(args.args())?),
        SyscallOp::Retype => retype(RetypeOp::try_from_args(args.args())?),
        SyscallOp::ThreadDispatch => dispatch(DispatchOp::try_from_args(args.args())?, &ctx),
        _ => Err(CapError::SyscallNotImplemented),
    }
}
