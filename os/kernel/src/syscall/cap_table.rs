use crate::{
    arch::{
        exec::ExecState,
        mem::{Frame, PhysAddr},
        ArchSystem, System,
    },
    caps::{trie::TrieSlotPayload, CapBlock, Capability},
    thread::Thread,
};
use crate::{
    caps::{CapTable, Resources, UserPtrTExt as _},
    kmem::KPtr,
};
use qapi::caps::{CapError, PositiveIsize};
use qapi::syscall::ops::ctable::{ConsKind, ConsOp, CopyOp, DropOp, LinkOp, ThreadCons, UnlinkOp};

use super::SyscallResp;

pub fn cap_table_cons(opts: ConsOp) -> SyscallResp {
    let ctable = Thread::get_cap(opts.table_cap.cap()).ok_or(CapError::CapNotFound)?;
    let ctable = ctable.as_ctable()?;

    match opts.kind {
        ConsKind::Thread => {
            let ThreadCons {
                entry,
                rsp,
                addrspace,
                caps,
                frame,
                arg0,
            } = opts.cons_args.cast::<ThreadCons>().verify()?.safe_read()?;
            let comp = Thread::get_cap(caps.cap()).ok_or(CapError::CapNotFound)?;
            let comp = comp.as_ctable()?;
            let addrspace = Thread::get_cap(addrspace.cap()).ok_or(CapError::CapNotFound)?;
            let addrspace = addrspace.as_addrspace()?;

            let thread = Thread::new(
                <ArchSystem as System>::ExecState::new_thread(entry, rsp, arg0),
                // SAFETY: It's okay to cast a CapBlock to a CapTable
                Resources::from_parts(addrspace.clone(), unsafe {
                    comp.cast_ref::<CapTable<ArchSystem>>().clone()
                }),
            );
            let frame = Frame::try_from_start_address(PhysAddr::try_new(frame)?)?;
            let thread = KPtr::new(frame, thread)?;
            CapBlock::at(ctable, opts.slot_id)
                .try_set(TrieSlotPayload::Data(Capability::Thread(thread)))?;
            Ok(PositiveIsize::zero())
        }
        ConsKind::CapTable => todo!(),
        ConsKind::TranscientPageTable => todo!(),
        ConsKind::Addrspace => todo!(),
        ConsKind::SyncCall => todo!(),
        ConsKind::SyncRet => todo!(),
    }
}

pub fn cap_table_copy(opts: CopyOp) -> SyscallResp {
    todo!();
}

pub fn cap_table_drop(opts: DropOp) -> SyscallResp {
    todo!();
}

pub fn cap_table_link(opts: LinkOp) -> SyscallResp {
    todo!();
}

pub fn cap_table_unlink(opts: UnlinkOp) -> SyscallResp {
    todo!();
}
