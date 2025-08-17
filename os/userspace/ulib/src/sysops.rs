use extend::ext;
use qapi::{
    caps::{CapError, SysSlot, ctable::CapTableCap, thread::ThreadCap, vmtable::PageTableCap},
    syscall::{
        SyscallArgs, SyscallOp, SyscallRequest as _,
        ops::{
            ctable::{ConsArgs, ConsKind, ConsOp, ThreadCons},
            retype::{RetypeKind, RetypeOp},
            thread::DispatchOp,
        },
    },
    types::UserPtr,
};
use zerocopy::IntoBytes as _;

use qapi::mem::Frame;

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
pub impl CapTableCap {
    #[allow(clippy::too_many_arguments)]
    fn make_thread(
        &self,
        slot: SysSlot,
        entry: extern "C" fn(usize) -> !,
        stack_top: *mut (),
        addrspace: PageTableCap,
        caps: CapTableCap,
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

    fn construct(&self, args: ConsArgs, slot: SysSlot) -> Result<(), CapError> {
        let (kind, args_bytes) = match &args {
            ConsArgs::Thread(thread_cons) => (ConsKind::Thread, thread_cons.as_bytes()),
            ConsArgs::TranscientPageTable(_page_table_cons) => todo!(),
            ConsArgs::Addrspace(_addrspace_cons) => todo!(),
            ConsArgs::SyncCall(_sync_call_cons) => todo!(),
            ConsArgs::SyncRet(_sync_ret_cons) => todo!(),
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
