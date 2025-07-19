use super::*;

use crate::syscall::ulib::syscall;

impl CapTable {
    pub fn construct(&self, args: ConsArgs, slot: SlotId<NUM_SLOTS>) -> Result<(), CapError> {
        let (kind, args_bytes) = match &args {
            ConsArgs::Thread(thread_cons) => (ConsKind::Thread, thread_cons.as_bytes()),
            ConsArgs::TranscientPageTable(page_table_cons) => todo!(),
            ConsArgs::Addrspace(addrspace_cons) => todo!(),
            ConsArgs::SyncCall(sync_call_cons) => todo!(),
            ConsArgs::SyncRet(sync_ret_cons) => todo!(),
            ConsArgs::CapTable(cap_table_cons) => todo!(),
        };

        let cons_op = ConsOp {
            slot_id: slot,
            kind,
            cons_args: UserPtr::from_addr(args_bytes.as_ptr() as usize),
        };
        syscall(self.0, cons_op).map(|_| ())
    }
}
