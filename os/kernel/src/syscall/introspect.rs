use core::mem::MaybeUninit;

use qapi::caps::PositiveIsize;
use qapi::syscall::ops::introspect::{
    CBlockInspect, CLinkInspect, IntrospectOp, IntrospectResult, KObj, ThreadInspect,
};

use super::SyscallResp;
use crate::arch::ArchCaps as _;
use crate::arch::mem::Frame;
use crate::caps::trie::TrieSlotPayload;
use crate::caps::{CapBlock, Capability, UserPtrMutTExt as _};
use crate::thread::Thread;

pub fn introspect(opts: IntrospectOp) -> SyscallResp {
    let cap = Thread::with_current(|thread| thread.get_cap(opts.cap));
    let write_buf = opts.write_buf.verify()?;
    let result = match cap.as_ref().map(|c| c.payload()) {
        Some(TrieSlotPayload::Data(Capability::Thread(thread))) => {
            IntrospectResult::Thread(KObj::new(thread.frame().into(), thread.introspect()))
        }
        Some(TrieSlotPayload::Data(Capability::CapBlock(kptr))) => {
            IntrospectResult::CBlock(KObj::new(kptr.frame().into(), CapBlock::introspect(kptr)))
        }
        Some(TrieSlotPayload::Data(Capability::SyncCall(sync_call))) => {
            IntrospectResult::SyncCall(sync_call.introspect())
        }

        Some(TrieSlotPayload::Data(Capability::CompResource(resources))) => {
            IntrospectResult::Resources(resources.introspect())
        }
        Some(TrieSlotPayload::Data(Capability::Arch(a))) => a.introspect(),
        Some(TrieSlotPayload::Link(kptr)) => {
            IntrospectResult::CLink(KObj::new(kptr.frame().into(), CapBlock::introspect(kptr)))
        }
        Some(TrieSlotPayload::Data(Capability::Notification(notification))) => {
            IntrospectResult::Notification(notification.introspect())
        }
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
