pub mod sync_endpoint;
pub use crate::make_sync_call;

use core::mem::MaybeUninit;

use extend::ext;
use qapi::caps::ctable::CTableCap;
use qapi::caps::resources::ResourcesCap;
use qapi::caps::sync_ipc::SyncInvokeCap;
use qapi::caps::thread::ThreadCap;
use qapi::caps::vmtable::VMTableCap;
use qapi::caps::{CapError, CapId, PositiveIsize, SysSlot};
use qapi::mem::{Frame, PageFlags};
use qapi::syscall::ops::ctable::{
    CTableCons, ConsArgs, ConsKind, ConsOp, CopyOp, DropOp, LinkOp, SyncCallCons, ThreadCons,
};
use qapi::syscall::ops::introspect::{IntrospectOp, IntrospectResult};
use qapi::syscall::ops::retype::{RetypeKind, RetypeOp};
use qapi::syscall::ops::sync_ipc::{SYNC_CALL_ARGS, SyncCallFun, SyncInvokeOp};
use qapi::syscall::ops::thread::DispatchOp;
use qapi::syscall::ops::vmtable::{PaddedPageTableOffset, VMLinkOp, VMSetAttr, VMUnlinkOp};
use qapi::syscall::{SyscallArgs, SyscallOp, SyscallRequest};
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
        resources: ResourcesCap,
        frame: Frame,
        arg0: usize,
    ) -> Result<(), CapError> {
        self.construct(
            ConsArgs::Thread(ThreadCons {
                entry: entry as usize,
                rsp: stack_top as usize,
                resources,
                frame,
                arg0,
                _padding: 0,
            }),
            slot,
        )
    }

    fn make_sync_call(
        &self,
        slot: SysSlot,
        fun: SyncCallFun,
        resources: ResourcesCap,
    ) -> Result<(), CapError> {
        self.construct(
            ConsArgs::SyncCall(SyncCallCons {
                entry: fun as usize,
                resources,
                _padding: 0,
            }),
            slot,
        )
    }

    fn make_ctable(&self, slot: SysSlot, frame: Frame) -> Result<(), CapError> {
        self.construct(ConsArgs::CTable(CTableCons { frame }), slot)
    }

    #[cfg(target_arch = "x86_64")]
    fn make_vmtable(&self, slot: SysSlot, frame: Frame, level: u8) -> Result<(), CapError> {
        use qapi::syscall::ops::ctable::VMTableCons;

        self.construct(
            ConsArgs::VMTable(VMTableCons {
                frame,
                level: level.into(),
            }),
            slot,
        )
    }

    fn construct(&self, args: ConsArgs, slot: SysSlot) -> Result<(), CapError> {
        let (kind, args_bytes) = match &args {
            ConsArgs::Thread(thread_cons) => (ConsKind::Thread, thread_cons.as_bytes()),
            ConsArgs::VMTable(vm_cons) => (ConsKind::VMTable, vm_cons.as_bytes()),
            ConsArgs::SyncCall(sync_call_cons) => (ConsKind::SyncCall, sync_call_cons.as_bytes()),
            ConsArgs::CTable(cap_table_cons) => (ConsKind::CTable, cap_table_cons.as_bytes()),
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

    fn link_at(&self, slot: SysSlot, cap: CTableCap) -> Result<(), CapError> {
        let args = LinkOp {
            top_table: *self,
            slot_id: slot,
            bottom_table: cap,
        };
        syscall(SyscallArgs::new_with_args(
            SyscallOp::CapLink,
            args.into_args(),
        ))
        .map(|_| ())
    }

    fn drop_at(&self, slot: SysSlot) -> Result<(), CapError> {
        let args = DropOp {
            table_cap: *self,
            slot_id: slot,
        };
        syscall(SyscallArgs::new_with_args(
            SyscallOp::CapDrop,
            args.into_args(),
        ))
        .map(|_| ())
    }

    fn copy_from(&self, slot: SysSlot, cap: CapId) -> Result<(), CapError> {
        cap.copy_into(*self, slot)
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

    fn copy_into(&self, to_table: CTableCap, to_slot: SysSlot) -> Result<(), CapError> {
        let args = CopyOp {
            from_cap: *self,
            to_table,
            to_slot,
        };
        syscall(SyscallArgs::new_with_args(
            SyscallOp::CapCopy,
            args.into_args(),
        ))
        .map(|_| ())
    }
}

#[ext]
pub impl VMTableCap {
    fn link_at(
        &self,
        offset: PaddedPageTableOffset,
        other: VMTableCap,
        flags: PageFlags,
    ) -> Result<(), CapError> {
        let op = VMLinkOp {
            top_table: *self,
            offset,
            bottom_table: other,
            flags,
        };
        syscall(SyscallArgs::new_with_args(
            SyscallOp::VMLink,
            op.into_args(),
        ))
        .map(|_| ())
    }

    fn unlink_at(&self, offset: PaddedPageTableOffset) -> Result<(), CapError> {
        let op = VMUnlinkOp {
            table: *self,
            offset,
        };
        syscall(SyscallArgs::new_with_args(
            SyscallOp::VMUnlink,
            op.into_args(),
        ))
        .map(|_| ())
    }

    fn set_attr(&self, offset: PaddedPageTableOffset, flags: PageFlags) -> Result<(), CapError> {
        let op = VMSetAttr {
            table: *self,
            offset,
            flags,
        };
        syscall(SyscallArgs::new_with_args(
            SyscallOp::VMSetAttr,
            op.into_args(),
        ))
        .map(|_| ())
    }
}
