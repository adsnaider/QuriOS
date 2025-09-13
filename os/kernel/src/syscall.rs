use ctable::{cap_table_cons, cap_table_copy, cap_table_drop, cap_table_link};
use introspect::introspect;
use irq::{irq_set, irq_unset};
use qapi::caps::sync_ipc::{ExceptionAbi, ExceptionArgs};
use qapi::caps::{CapError, PositiveIsize};
use qapi::syscall::ops::ctable::{ConsOp, CopyOp, DropOp, LinkOp};
use qapi::syscall::ops::introspect::IntrospectOp;
use qapi::syscall::ops::irq::{IrqSet, IrqUnset};
use qapi::syscall::ops::notify::NotifyOp;
use qapi::syscall::ops::retype::RetypeOp;
use qapi::syscall::ops::sync_ipc::{SyncInvokeOp, SyncRetOp};
use qapi::syscall::ops::thread::DispatchOp;
use qapi::syscall::ops::vmtable::{VMLinkOp, VMSetAttr, VMUnlinkOp};
use qapi::syscall::{SyscallArgs, SyscallArgsInit, SyscallOp, SyscallRequest};
use retyping::retype;
use sync_ipc::{sync_invoke, sync_ret};
use thread::{dispatch, thread_sig_wait};
use vmtable::{vm_link, vm_set_attr, vm_unlink};

use crate::arch::{ArchSystem, InvokeAbi, System};
use crate::caps::ExceptionHandler;
use crate::never::Never;
use crate::thread::{Thread, ThreadExecCtx};

mod ctable;
mod introspect;
mod irq;
mod notify;
mod retyping;
mod sync_ipc;
mod thread;
mod vmtable;

pub type SyscallResp = Result<PositiveIsize, CapError>;

pub fn syscall_handler(
    args: SyscallArgs<SyscallArgsInit>,
    ctx: <ArchSystem as System>::IrqCtx,
) -> SyscallResp {
    // SAFETY: We are allowed to get a mutable reference at the start of the syscall. It will get dropped
    log::trace!("Syscall ctx: {ctx:#?}");

    let op = args.op()?;
    log::debug!("Sys Op: {op:?}");
    match op {
        SyscallOp::CapCons => cap_table_cons(ConsOp::try_from_args(args.args())?),
        SyscallOp::CapDrop => cap_table_drop(DropOp::try_from_args(args.args())?),
        SyscallOp::CapCopy => cap_table_copy(CopyOp::try_from_args(args.args())?),
        SyscallOp::CapLink => cap_table_link(LinkOp::try_from_args(args.args())?),
        SyscallOp::Retype => retype(RetypeOp::try_from_args(args.args())?),
        SyscallOp::ThreadDispatch => dispatch(DispatchOp::try_from_args(args.args())?, ctx),
        SyscallOp::VMLink => vm_link(VMLinkOp::try_from_args(args.args())?),
        SyscallOp::VMUnlink => vm_unlink(VMUnlinkOp::try_from_args(args.args())?),
        SyscallOp::VMSetAttr => vm_set_attr(VMSetAttr::try_from_args(args.args())?),
        SyscallOp::SyncInvoke => sync_invoke(SyncInvokeOp::try_from_args(args.args())?, ctx),
        SyscallOp::SyncRet => sync_ret(SyncRetOp::try_from_args(args.args())?, ctx),
        SyscallOp::Introspect => introspect(IntrospectOp::try_from_args(args.args())?),
        SyscallOp::Notify => notify::notify(NotifyOp::try_from_args(args.args())?, ctx),
        SyscallOp::IrqSet => irq_set(IrqSet::try_from_args(args.args())?),
        SyscallOp::IrqUnset => irq_unset(IrqUnset::try_from_args(args.args())?),
        SyscallOp::ThreadSigWait => thread_sig_wait(ctx),
    }
}

pub fn ring3_exception_handler(args: ExceptionArgs, ctx: <ArchSystem as System>::IrqCtx) -> Never {
    let dispatcher;
    {
        let exception_handler = Thread::with_current(|thread| thread.exception_handler());
        let abi = ExceptionAbi;
        let exception_ctx = match exception_handler {
            ExceptionHandler::Within { entry } => {
                let resources = Thread::with_current(|thread| thread.resources());
                let exec_state = abi.invoke_with_args(args, entry);
                ThreadExecCtx::new(exec_state, resources, abi)
            }
        };
        // TODO: If this fails, notifiy a scheduler thread instead...
        dispatcher = Thread::with_current(move |thread| thread.invoke(exception_ctx, ctx)).unwrap();
    }
    dispatcher.dispatch();
}
