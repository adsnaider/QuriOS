use qapi::caps::{CapError, PositiveIsize};
use qapi::syscall::ops::ctable::{
    CTableCons, ConsKind, ConsOp, CopyOp, DropOp, LinkOp, SyncCallCons, ThreadCons, VMTableCons,
};

use super::SyscallResp;
use crate::arch::{ArchCaps as _, ArchSystem, ExecState, System};
use crate::caps::trie::TrieSlotPayload;
use crate::caps::{CapBlock, Capability, UserPtrTExt};
use crate::kmem::KPtr;
use crate::sync_call::SyncCall;
use crate::thread::Thread;

pub fn cap_table_cons(opts: ConsOp) -> SyscallResp {
    let ctable = Thread::with_current(|thread| thread.get_cap(opts.table_cap.cap()))
        .unwrap()
        .ok_or(CapError::CapNotFound)?;
    let ctable = ctable.as_ctable()?;

    match opts.kind {
        ConsKind::Thread => {
            let ThreadCons {
                entry,
                rsp,
                resources,
                frame,
                arg0,
                ..
            } = opts.cons_args.cast::<ThreadCons>().verify()?.safe_read()?;
            let resources = Thread::with_current(|thread| thread.get_cap(resources.cap()))
                .unwrap()
                .ok_or(CapError::CapNotFound)?;
            let resources = resources.as_resources()?;

            let thread = Thread::new(
                <ArchSystem as System>::ExecState::new_thread(entry, rsp, arg0),
                // SAFETY: It's okay to cast a CapBlock to a CapTable
                resources.clone(),
            );
            let thread = KPtr::new(frame.into(), thread)?;
            CapBlock::at(ctable, opts.slot_id)
                .try_set(TrieSlotPayload::Data(Capability::Thread(thread)))?;
            Ok(PositiveIsize::zero())
        }
        ConsKind::CTable => {
            let CTableCons { frame } = opts.cons_args.cast::<CTableCons>().verify()?.safe_read()?;
            let new_ctable = CapBlock::empty();
            let new_ctable = KPtr::new(frame.into(), new_ctable)?;
            CapBlock::at(ctable, opts.slot_id)
                .try_set(TrieSlotPayload::Data(Capability::CapBlock(new_ctable)))?;
            Ok(PositiveIsize::zero())
        }
        ConsKind::VMTable => {
            let args = opts.cons_args.cast::<VMTableCons>().verify()?.safe_read()?;
            let acap = <ArchSystem as System>::ArchCaps::new_vmtable(args)?;
            CapBlock::at(ctable, opts.slot_id)
                .try_set(TrieSlotPayload::Data(Capability::Arch(acap)))?;
            Ok(PositiveIsize::zero())
        }
        ConsKind::SyncCall => {
            let SyncCallCons {
                entry, resources, ..
            } = opts
                .cons_args
                .cast::<SyncCallCons>()
                .verify()?
                .safe_read()?;
            let resources = Thread::with_current(|thread| thread.get_cap(resources.cap()))
                .unwrap()
                .ok_or(CapError::CapNotFound)?;
            // SAFETY: Casting a ctable to resources is fine.
            let resources = resources.as_resources()?;
            CapBlock::at(ctable, opts.slot_id).try_set(TrieSlotPayload::Data(
                Capability::SyncCall(SyncCall::new(resources.clone(), entry, Default::default())),
            ))?;
            Ok(PositiveIsize::zero())
        }
    }
}

pub fn cap_table_copy(opts: CopyOp) -> SyscallResp {
    let to_table = Thread::with_current(|thread| thread.get_cap(opts.to_table.cap()))
        .unwrap()
        .ok_or(CapError::CapNotFound)?;
    let to_table = to_table.as_ctable()?;

    let from_cap = Thread::with_current(|thread| thread.get_cap(opts.from_cap))
        .unwrap()
        .ok_or(CapError::CapNotFound)?
        .data()
        .ok_or(CapError::CapNotFound)?
        .clone();

    CapBlock::at(to_table, opts.to_slot).try_set(TrieSlotPayload::Data(from_cap))?;
    Ok(PositiveIsize::zero())
}

pub fn cap_table_drop(opts: DropOp) -> SyscallResp {
    let ctable = Thread::with_current(|thread| thread.get_cap(opts.table_cap.cap()))
        .unwrap()
        .ok_or(CapError::CapNotFound)?;
    let ctable = ctable.as_ctable()?;
    CapBlock::at(ctable, opts.slot_id).kill();
    Ok(PositiveIsize::zero())
}

pub fn cap_table_link(opts: LinkOp) -> SyscallResp {
    let top_table = Thread::with_current(|thread| thread.get_cap(opts.top_table.cap()))
        .unwrap()
        .ok_or(CapError::CapNotFound)?;
    let top_table = top_table.as_ctable()?;

    let bottom_table = Thread::with_current(|thread| thread.get_cap(opts.bottom_table.cap()))
        .unwrap()
        .ok_or(CapError::CapNotFound)?;
    let bottom_table = bottom_table.as_ctable()?;

    CapBlock::at(top_table, opts.slot_id).try_set(TrieSlotPayload::Link(bottom_table.clone()))?;
    Ok(PositiveIsize::zero())
}
