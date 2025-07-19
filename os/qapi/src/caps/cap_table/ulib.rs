use super::*;

use crate::syscall::ulib::syscall;

impl CapTable {
    pub fn construct(&self, args: ConsArgs, slot: SlotId<NUM_SLOTS>) -> Result<(), CapError> {
        let (kind, args_bytes) = match &args {
            ConsArgs::Thread(thread_cons) => (ConsKind::Thread, thread_cons.as_bytes()),
            ConsArgs::TranscientPageTable(_page_table_cons) => todo!(),
            ConsArgs::Addrspace(_addrspace_cons) => todo!(),
            ConsArgs::SyncCall(_sync_call_cons) => todo!(),
            ConsArgs::SyncRet(_sync_ret_cons) => todo!(),
            ConsArgs::CapTable(_cap_table_cons) => todo!(),
        };

        let cons_op = ConsOp {
            slot_id: slot,
            kind,
            cons_args: UserPtr::from_addr(args_bytes.as_ptr() as usize),
        };
        syscall(self.0, cons_op).map(|_| ())
    }
}
