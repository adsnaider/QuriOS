use core::mem::MaybeUninit;

use qapi::caps::PositiveIsize;
use qapi::syscall::ops::introspect::{IntrospectOp, IntrospectResult};

use super::SyscallResp;
use crate::arch::ArchCaps as _;
use crate::arch::mem::Frame;
use crate::caps::trie::TrieSlotPayload;
use crate::caps::{CapBlock, Capability, UserPtrMutTExt as _};
use crate::thread::Thread;

pub fn introspect(opts: IntrospectOp) -> SyscallResp {
    let cap = Thread::with_current(|thread| thread.get_cap(opts.cap)).unwrap();
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
        Some(TrieSlotPayload::Data(Capability::CompResource(resources))) => resources.introspect(),
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
