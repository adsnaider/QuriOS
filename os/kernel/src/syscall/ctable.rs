use qapi::caps::{CapError, PositiveIsize};
use qapi::syscall::ops::ctable::{
    ConsKind, ConsOp, CopyOp, DropOp, LinkOp, SyncCallCons, ThreadCons,
};

use super::SyscallResp;
use crate::arch::exec::ExecState;
use crate::arch::mem::{Frame, PhysAddr};
use crate::arch::{ArchSystem, System};
use crate::caps::trie::TrieSlotPayload;
use crate::caps::{CapBlock, CapTable, Capability, Resources, UserPtrTExt};
use crate::kmem::KPtr;
use crate::sync_call::SyncCall;
use crate::thread::Thread;

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
        ConsKind::CTable => todo!(),
        ConsKind::VMTable => todo!(),
        ConsKind::SyncCall => {
            let SyncCallCons {
                entry,
                cspace,
                vmspace,
            } = opts
                .cons_args
                .cast::<SyncCallCons>()
                .verify()?
                .safe_read()?;
            let cspace = Thread::get_cap(cspace.cap()).ok_or(CapError::CapNotFound)?;
            // SAFETY: Casting a ctable to cspace is fine.
            let cspace: &KPtr<CapTable<ArchSystem>> = unsafe { cspace.as_ctable()?.cast_ref() };
            let vmspace = Thread::get_cap(vmspace.cap()).ok_or(CapError::CapNotFound)?;
            let vmspace = vmspace.as_addrspace()?;
            CapBlock::at(ctable, opts.slot_id).try_set(TrieSlotPayload::Data(
                Capability::SyncCall(SyncCall::new(
                    Resources::from_parts(vmspace.clone(), cspace.clone()),
                    entry,
                )),
            ))?;
            Ok(PositiveIsize::zero())
        }
    }
}

pub fn cap_table_copy(opts: CopyOp) -> SyscallResp {
    let to_table = Thread::get_cap(opts.to_table.cap()).ok_or(CapError::CapNotFound)?;
    let to_table = to_table.as_ctable()?;

    let from_cap = Thread::get_cap(opts.from_cap)
        .ok_or(CapError::CapNotFound)?
        .data()
        .ok_or(CapError::CapNotFound)?
        .clone();

    CapBlock::at(to_table, opts.to_slot).try_set(TrieSlotPayload::Data(from_cap))?;
    Ok(PositiveIsize::zero())
}

pub fn cap_table_drop(opts: DropOp) -> SyscallResp {
    let ctable = Thread::get_cap(opts.table_cap.cap()).ok_or(CapError::CapNotFound)?;
    let ctable = ctable.as_ctable()?;
    CapBlock::at(ctable, opts.slot_id).kill();
    Ok(PositiveIsize::zero())
}

pub fn cap_table_link(opts: LinkOp) -> SyscallResp {
    let top_table = Thread::get_cap(opts.top_table.cap()).ok_or(CapError::CapNotFound)?;
    let top_table = top_table.as_ctable()?;

    let bottom_table = Thread::get_cap(opts.bottom_table.cap()).ok_or(CapError::CapNotFound)?;
    let bottom_table = bottom_table.as_ctable()?;

    CapBlock::at(top_table, opts.slot_id).try_set(TrieSlotPayload::Link(bottom_table.clone()))?;
    Ok(PositiveIsize::zero())
}
