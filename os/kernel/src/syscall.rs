use cap_table::{cap_table_cons, cap_table_copy, cap_table_drop, cap_table_link};
use qapi::caps::{CapError, PositiveIsize};
use qapi::syscall::ops::ctable::{ConsOp, CopyOp, DropOp, LinkOp};
use qapi::syscall::ops::retype::RetypeOp;
use qapi::syscall::ops::thread::DispatchOp;
use qapi::syscall::{SyscallArgs, SyscallArgsInit, SyscallOp, SyscallRequest as _};
use retyping::retype;
use thread::dispatch;

use crate::arch::exec::ExecState;
use crate::arch::{ArchSystem, System};

mod cap_table;
mod retyping;
mod thread;

pub type SyscallResp = Result<PositiveIsize, CapError>;

pub fn syscall_handler(
    args: SyscallArgs<SyscallArgsInit>,
    ctx: <<ArchSystem as System>::ExecState as ExecState>::RegCtx,
) -> SyscallResp {
    // SAFETY: We are allowed to get a mutable reference at the start of the syscall. It will get dropped
    log::trace!("Handling syscall: {args:?} {ctx:#?}");

    let op = args.op()?;
    match op {
        SyscallOp::CapCons => cap_table_cons(ConsOp::try_from_args(args.args())?),
        SyscallOp::CapDrop => cap_table_drop(DropOp::try_from_args(args.args())?),
        SyscallOp::CapCopy => cap_table_copy(CopyOp::try_from_args(args.args())?),
        SyscallOp::CapLink => cap_table_link(LinkOp::try_from_args(args.args())?),
        SyscallOp::Retype => retype(RetypeOp::try_from_args(args.args())?),
        SyscallOp::ThreadDispatch => dispatch(DispatchOp::try_from_args(args.args())?, &ctx),
        SyscallOp::PageTableLink => todo!(),
        SyscallOp::PageTableUnlink => todo!(),
        SyscallOp::PageTableUpdateFlags => todo!(),
        SyscallOp::SyncInvoke => todo!(),
        SyscallOp::SyncRet => todo!(),
        SyscallOp::Introspect => todo!(),
        _ => Err(CapError::SyscallNotImplemented),
    }
}
