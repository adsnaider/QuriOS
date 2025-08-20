use core::mem::MaybeUninit;

use extend::ext;
use qapi::caps::ctable::CTableCap;
use qapi::caps::sync_ipc::SyncInvokeCap;
use qapi::caps::thread::ThreadCap;
use qapi::caps::vmtable::VMTableCap;
use qapi::caps::{CapError, CapId, PositiveIsize, SysSlot};
use qapi::mem::Frame;
use qapi::syscall::ops::ctable::{ConsArgs, ConsKind, ConsOp, SyncCallCons, ThreadCons};
use qapi::syscall::ops::introspect::{IntrospectOp, IntrospectResult};
use qapi::syscall::ops::retype::{RetypeKind, RetypeOp};
use qapi::syscall::ops::sync_ipc::{SYNC_CALL_ARGS, SyncCallFun, SyncInvokeOp};
use qapi::syscall::ops::thread::DispatchOp;
use qapi::syscall::{self, SyscallArgs, SyscallOp, SyscallRequest};
use qapi::types::{UserPtr, UserPtrMut};
use zerocopy::IntoBytes as _;

use crate::syscall::syscall;

#[ext]
pub impl Frame {
    fn retype(&self, to: RetypeKind) -> Result<(), CapError> {
        let op = SyscallOp::Retype;
        let args = RetypeOp {
            frame: self.base(),
            to,
        }
        .into_args();
        syscall(SyscallArgs::new_with_args(op, args)).map(|_| ())
    }
}

#[ext]
pub impl ThreadCap {
    fn dispatch(&self) -> Result<(), CapError> {
        let args = DispatchOp { thread_cap: *self }.into_args();
        syscall(SyscallArgs::new_with_args(SyscallOp::ThreadDispatch, args)).map(|_| ())
    }
}

#[ext]
pub impl CTableCap {
    #[allow(clippy::too_many_arguments)]
    fn make_thread(
        &self,
        slot: SysSlot,
        entry: extern "C" fn(usize) -> !,
        stack_top: *mut (),
        addrspace: VMTableCap,
        caps: CTableCap,
        frame: Frame,
        arg0: usize,
    ) -> Result<(), CapError> {
        self.construct(
            ConsArgs::Thread(ThreadCons {
                entry: entry as usize,
                rsp: stack_top as usize,
                addrspace,
                caps,
                frame: frame.base(),
                arg0,
            }),
            slot,
        )
    }

    fn make_sync_call(
        &self,
        slot: SysSlot,
        fun: SyncCallFun,
        vmspace: VMTableCap,
        cspace: CTableCap,
    ) -> Result<(), CapError> {
        self.construct(
            ConsArgs::SyncCall(SyncCallCons {
                entry: fun as usize,
                cspace,
                vmspace,
            }),
            slot,
        )
    }

    fn construct(&self, args: ConsArgs, slot: SysSlot) -> Result<(), CapError> {
        let (kind, args_bytes) = match &args {
            ConsArgs::Thread(thread_cons) => (ConsKind::Thread, thread_cons.as_bytes()),
            ConsArgs::VMTable(_vm_cons) => todo!(),
            ConsArgs::SyncCall(sync_call_cons) => (ConsKind::SyncCall, sync_call_cons.as_bytes()),
            ConsArgs::CapTable(_cap_table_cons) => todo!(),
        };

        let args = ConsOp {
            table_cap: *self,
            slot_id: slot,
            kind,
            cons_args: UserPtr::from_addr(args_bytes.as_ptr() as usize),
        };
        syscall(SyscallArgs::new_with_args(
            SyscallOp::CapCons,
            args.into_args(),
        ))
        .map(|_| ())
    }
}

#[ext]
pub impl SyncInvokeCap {
    fn invoke(
        &self,
        args: [MaybeUninit<usize>; SYNC_CALL_ARGS],
    ) -> Result<PositiveIsize, CapError> {
        let args = SyncInvokeOp { cap: *self, args };
        syscall(SyscallArgs::new_with_args(
            SyscallOp::SyncInvoke,
            args.into_args(),
        ))
    }
}

#[ext]
pub impl CapId {
    fn introspect(&self) -> Result<IntrospectResult, CapError> {
        let mut out = MaybeUninit::uninit();
        let op = IntrospectOp {
            cap: *self,
            write_buf: UserPtrMut::new(&mut out as *mut MaybeUninit<IntrospectResult>),
        };
        syscall(SyscallArgs::new_with_args(
            SyscallOp::Introspect,
            op.into_args(),
        ))
        // SAFETY: If the syscall is successful, the kernel can be trusted to set reasonable bytes
        .map(|_| unsafe { out.assume_init() })
    }
}
