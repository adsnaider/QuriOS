use super::*;

use crate::{
    mem::Frame,
    syscall::{SyscallArgs, SyscallOp, ulib::syscall},
};

impl CapTableCap {
    #[allow(clippy::too_many_arguments)]
    pub fn make_thread(
        &self,
        slot: SlotId<NUM_SLOTS>,
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

    pub fn construct(&self, args: ConsArgs, slot: SlotId<NUM_SLOTS>) -> Result<(), CapError> {
        let (kind, args_bytes) = match &args {
            ConsArgs::Thread(thread_cons) => (ConsKind::Thread, thread_cons.as_bytes()),
            ConsArgs::TranscientPageTable(_page_table_cons) => todo!(),
            ConsArgs::Addrspace(_addrspace_cons) => todo!(),
            ConsArgs::SyncCall(_sync_call_cons) => todo!(),
            ConsArgs::SyncRet(_sync_ret_cons) => todo!(),
            ConsArgs::CapTable(_cap_table_cons) => todo!(),
        };

        let args = ConsOp {
            table_cap: self.0,
            slot_id: slot,
            kind,
            cons_args: UserPtr::from_addr(args_bytes.as_ptr() as usize),
        }
        .into_args();
        syscall(SyscallArgs::new_with_args(SyscallOp::CapTableCons, args)).map(|_| ())
    }
}
