pub mod sync_endpoint;
use core::mem::MaybeUninit;

use extend::ext;
use qapi::caps::ctable::CTableCap;
use qapi::caps::irq_ctrl::IrqCtrlCap;
use qapi::caps::notify::NotificationCap;
use qapi::caps::resources::ResourcesCap;
use qapi::caps::sync_ipc::SyncInvokeCap;
use qapi::caps::thread::ThreadCap;
use qapi::caps::vmtable::VMTableCap;
use qapi::caps::{CapError, CapId, PositiveIsize, SysSlot};
use qapi::mem::{Frame, PageFlags};
use qapi::syscall::ops::ctable::{
    CTableCons, ConsArgs, ConsKind, ConsOp, CopyOp, DropOp, LinkOp, NotificationCons, SyncCallCons,
    ThreadCons,
};
use qapi::syscall::ops::introspect::{
    CBlockInspect, IntrospectOp, IntrospectResult, IrqCtrlInspect, KObj, NotificationInspect,
    ResourcesInspect, SyncCallInspect, ThreadInspect, VMTableInspect,
};
use qapi::syscall::ops::irq::{IrqSet, IrqUnset};
use qapi::syscall::ops::retype::{RetypeKind, RetypeOp};
use qapi::syscall::ops::sync_ipc::{SYNC_CALL_ARGS, SyncInvokeOp};
use qapi::syscall::ops::thread::DispatchOp;
use qapi::syscall::ops::vmtable::{
    PaddedPageTableOffset, VMLinkOp, VMMapOp, VMSetAttr, VMUnlinkOp, VMUnmapOp,
};
use qapi::syscall::{SyscallArgs, SyscallOp, SyscallRequest};
use qapi::types::{UserPtr, UserPtrMut};
use zerocopy::IntoBytes as _;

use crate::syscall::syscall;

pub trait Introspect {
    type Output;

    fn introspect(&self) -> Result<Self::Output, CapError>;
}

impl Introspect for CapId {
    type Output = IntrospectResult;

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

impl Introspect for CTableCap {
    type Output = KObj<CBlockInspect>;

    fn introspect(&self) -> Result<Self::Output, CapError> {
        let IntrospectResult::CBlock(block) = self.cap().introspect()? else {
            return Err(CapError::InvalidCapType);
        };
        Ok(block)
    }
}

impl Introspect for ThreadCap {
    type Output = KObj<ThreadInspect>;

    fn introspect(&self) -> Result<Self::Output, CapError> {
        let IntrospectResult::Thread(thread) = self.cap().introspect()? else {
            return Err(CapError::InvalidCapType);
        };
        Ok(thread)
    }
}

impl Introspect for SyncInvokeCap {
    type Output = SyncCallInspect;

    fn introspect(&self) -> Result<Self::Output, CapError> {
        let IntrospectResult::SyncCall(sync_call) = self.cap().introspect()? else {
            return Err(CapError::InvalidCapType);
        };
        Ok(sync_call)
    }
}
impl Introspect for NotificationCap {
    type Output = NotificationInspect;

    fn introspect(&self) -> Result<Self::Output, CapError> {
        let IntrospectResult::Notification(notification) = self.cap().introspect()? else {
            return Err(CapError::InvalidCapType);
        };
        Ok(notification)
    }
}

impl Introspect for ResourcesCap {
    type Output = ResourcesInspect;

    fn introspect(&self) -> Result<Self::Output, CapError> {
        let IntrospectResult::Resources(resources) = self.cap().introspect()? else {
            return Err(CapError::InvalidCapType);
        };
        Ok(resources)
    }
}

impl Introspect for VMTableCap {
    type Output = KObj<VMTableInspect>;

    fn introspect(&self) -> Result<Self::Output, CapError> {
        let IntrospectResult::VMTable(vmtable) = self.cap().introspect()? else {
            return Err(CapError::InvalidCapType);
        };
        Ok(vmtable)
    }
}

impl Introspect for IrqCtrlCap {
    type Output = IrqCtrlInspect;

    fn introspect(&self) -> Result<Self::Output, CapError> {
        let IntrospectResult::IrqCtrl(irq_ctrl) = self.cap().introspect()? else {
            return Err(CapError::InvalidCapType);
        };
        Ok(irq_ctrl)
    }
}

#[ext]
pub impl Frame {
    fn retype(&self, to: RetypeKind) -> Result<(), CapError> {
        let op = SyscallOp::Retype;
        let args = RetypeOp { frame: *self, to }.into_args();
        syscall(SyscallArgs::new_with_args(op, args)).map(|_| ())
    }
}

#[ext]
pub impl ThreadCap {
    fn dispatch(&self) -> Result<(), CapError> {
        let args = DispatchOp { thread_cap: *self }.into_args();
        syscall(SyscallArgs::new_with_args(SyscallOp::ThreadDispatch, args)).map(|_| ())
    }

    fn sig_wait() -> Result<u32, CapError> {
        syscall(SyscallArgs::new_uninit(SyscallOp::ThreadSigWait))
            .map(|sigs| sigs.try_into().unwrap())
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
        priority: u32,
        parent: ThreadCap,
    ) -> Result<(), CapError> {
        self.construct(
            ConsArgs::Thread(ThreadCons {
                entry: entry as usize,
                rsp: stack_top as usize,
                resources,
                frame,
                arg0,
                priority,
                parent,
                _padding: 0,
            }),
            slot,
        )
    }

    fn make_notification(
        &self,
        slot: SysSlot,
        thread: ThreadCap,
        badge: u32,
    ) -> Result<(), CapError> {
        self.construct(
            ConsArgs::Notification(NotificationCons { thread, badge }),
            slot,
        )
    }

    fn make_sync_call(
        &self,
        slot: SysSlot,
        fun: extern "C" fn(),
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
            ConsArgs::Notification(notification_cons) => {
                (ConsKind::Notification, notification_cons.as_bytes())
            }
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

    fn map_at(
        &self,
        offset: PaddedPageTableOffset,
        frame: Frame,
        flags: PageFlags,
    ) -> Result<(), CapError> {
        let op = VMMapOp {
            table: *self,
            offset,
            frame,
            flags,
        };
        syscall(SyscallArgs::new_with_args(SyscallOp::VMMap, op.into_args())).map(|_| ())
    }

    fn unmap_at(&self, offset: PaddedPageTableOffset) -> Result<(), CapError> {
        let op = VMUnmapOp {
            table: *self,
            offset,
        };
        syscall(SyscallArgs::new_with_args(
            SyscallOp::VMUnmap,
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

#[ext]
pub impl IrqCtrlCap {
    fn irq_set(&self, notification: NotificationCap, irq: usize) -> Result<(), CapError> {
        let op = IrqSet {
            irq_ctrl: *self,
            notification,
            irq,
        };
        syscall(SyscallArgs::new_with_args(
            SyscallOp::IrqSet,
            op.into_args(),
        ))
        .map(|_| ())
    }

    fn irq_unset(&self, irq: usize) -> Result<(), CapError> {
        let op = IrqUnset {
            irq_ctrl: *self,
            irq,
        };
        syscall(SyscallArgs::new_with_args(
            SyscallOp::IrqUnset,
            op.into_args(),
        ))
        .map(|_| ())
    }
}
