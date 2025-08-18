use core::mem::MaybeUninit;

use qapi::{
    caps::PositiveIsize,
    syscall::ops::introspect::{IntrospectOp, IntrospectResult},
};

use crate::{
    arch::{mem::Frame, ArchCaps as _},
    caps::{trie::TrieSlotPayload, CapBlock, Capability, UserPtrMutTExt as _},
    thread::Thread,
};

use super::SyscallResp;

pub fn introspect(opts: IntrospectOp) -> SyscallResp {
    let cap = Thread::get_cap(opts.cap);
    let write_buf = opts.write_buf.verify()?;
    let result = match cap.as_ref().map(|c| c.payload()) {
        Some(TrieSlotPayload::Data(Capability::Thread(thread))) => IntrospectResult::Thread {
            kobj: thread.frame().into(),
            thread: thread.introspect(),
        },
        Some(TrieSlotPayload::Data(Capability::CapBlock(kptr))) => IntrospectResult::CBlock {
            kobj: kptr.frame().into(),
            cblock: CapBlock::introspect(kptr),
        },
        Some(TrieSlotPayload::Data(Capability::SyncCall(sync_call))) => sync_call.introspect(),
        Some(TrieSlotPayload::Data(Capability::SyncRet(sync_ret))) => sync_ret.introspect(),
        Some(TrieSlotPayload::Data(Capability::Arch(a))) => a.introspect(),
        Some(TrieSlotPayload::Link(kptr)) => IntrospectResult::CLink {
            kobj: kptr.frame().into(),
        },
        None => IntrospectResult::Empty,
    };
    write_buf.safe_write(MaybeUninit::new(result))?;
    Ok(PositiveIsize::zero())
}

impl From<Frame> for qapi::mem::Frame {
    fn from(value: Frame) -> Self {
        Self::try_new(value.base().as_u64()).unwrap()
    }
}
