#![allow(unused)]
use alloc::collections::btree_map::BTreeMap;
use core::marker::PhantomData;

use derive_more::{Display, Error};
use qapi::caps::ctable::CTableCap;
use qapi::caps::notify::NotificationCap;
use qapi::caps::resources::ResourcesCap;
use qapi::caps::slotid::NUM_SLOTS;
use qapi::caps::sync_ipc::SyncInvokeCap;
use qapi::caps::thread::ThreadCap;
#[cfg(target_arch = "x86_64")]
use qapi::caps::vmtable::VMTableCap;
use qapi::caps::{CapError, CapId, SysSlot};
use qapi::mem::Frame;
use qapi::syscall::ops::ctable::ConsArgs;

use super::allocman::Resources;
use crate::caps::CPath;
use crate::sysops::CTableCapExt;

#[derive(Debug, Error, Display, Clone)]
pub enum CAllocError {
    #[display("Capability tree is full and can't allocate in the current execution stack")]
    CapTreeFull,
}

pub(super) trait CSpace {
    fn alloc_cap(&mut self, resources: &mut Resources) -> Result<CapSlot, CAllocError>;
    unsafe fn cap_free(&mut self, cap: CapId, resources: &mut Resources);
}

pub struct CapSlot {
    cap: CapId,
    parent: CTableCap,
}

impl CapSlot {
    pub fn cap(&self) -> CapId {
        self.cap
    }

    pub fn slot(&self) -> SysSlot {
        CPath::new(self.cap).split_end().1
    }

    pub fn construct(self, args: ConsArgs) -> Result<CapId, CapError> {
        self.parent.construct(args, self.slot())?;
        Ok(self.cap)
    }

    fn link_to(self, table: CTableCap) -> Result<CapId, CapError> {
        self.parent.link_at(self.slot(), table)?;
        Ok(self.cap)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn make_thread(
        self,
        entry: extern "C" fn(usize) -> !,
        stack_top: *mut (),
        resources: ResourcesCap,
        frame: Frame,
        arg0: usize,
        priority: u32,
        parent: ThreadCap,
    ) -> Result<ThreadCap, CapError> {
        self.parent.make_thread(
            self.slot(),
            entry,
            stack_top,
            resources,
            frame,
            arg0,
            priority,
            parent,
        )?;
        Ok(ThreadCap::new(self.cap))
    }
    pub fn make_notification(
        self,
        thread: ThreadCap,
        badge: u32,
    ) -> Result<NotificationCap, CapError> {
        self.parent.make_notification(self.slot(), thread, badge)?;
        Ok(NotificationCap::new(self.cap))
    }

    pub fn make_sync_call(
        self,
        fun: extern "C" fn(),
        resources: ResourcesCap,
    ) -> Result<SyncInvokeCap, CapError> {
        self.parent.make_sync_call(self.slot(), fun, resources)?;
        Ok(SyncInvokeCap::new(self.cap))
    }

    pub fn make_ctable(self, frame: Frame) -> Result<CTableCap, CapError> {
        self.parent.make_ctable(self.slot(), frame)?;
        Ok(CTableCap::new(self.cap))
    }

    #[cfg(target_arch = "x86_64")]
    pub fn make_vmtable(self, frame: Frame, level: u8) -> Result<VMTableCap, CapError> {
        self.parent.make_vmtable(self.slot(), frame, level)?;
        Ok(VMTableCap::new(self.cap))
    }
}

pub struct CapabilityMan {
    table_path: CPath,
    next_slot: SysSlot,
    table_caps: BTreeMap<CPath, CTableCap>,
    root_table_cap: CTableCap,
}

impl CapabilityMan {
    pub fn new(caps: CTableCap) -> Self {
        Self::new_starting_at(caps, SysSlot::try_new(1).unwrap())
    }

    pub const fn new_starting_at(caps: CTableCap, first_free: SysSlot) -> Self {
        let last_slot = match first_free.checked_add(-1) {
            Some(val) => val,
            None => SysSlot::zero(),
        };
        Self {
            table_caps: BTreeMap::new(),
            table_path: CPath::empty(),
            next_slot: last_slot,
            root_table_cap: caps,
        }
    }

    fn maybe_init(&mut self) {
        if self.table_caps.is_empty() {
            self.table_caps.insert(CPath::empty(), self.root_table_cap);
        }
    }

    pub fn alloc_cap(&mut self, resources: &mut Resources) -> Result<CapSlot, CAllocError> {
        self.maybe_init();
        match self.next_slot.checked_add(1) {
            Some(next_slot) => self.next_slot = next_slot,
            None => {
                let next_table_cap = resources.steal_cap()?;
                let frame = resources
                    .steal_frame()
                    .expect("Out of memory while allocating capability");
                let next_table_cap = next_table_cap
                    .make_ctable(frame)
                    .expect("Error allocating capability table");

                let next_table_slot = resources
                    .steal_cap()?
                    .link_to(next_table_cap)
                    .expect("Error linking CTables for capability extension");
                self.table_path = CPath::new(next_table_slot);
                self.table_caps.insert(self.table_path, next_table_cap);
                self.next_slot = SysSlot::zero();
            }
        }
        let cap = self.table_path.push(self.next_slot);
        let parent = self
            .table_caps
            .get(&cap.split_end().0)
            .expect("Missing parent table cap for allocated capability");
        Ok(CapSlot {
            cap: cap.cap(),
            parent: *parent,
        })
    }

    pub fn cap_free(&mut self, cap: CapId, resources: &mut Resources) {
        // TODO: Linked list of freed-up caps?
    }
}

impl CSpace for CapabilityMan {
    fn alloc_cap(&mut self, resources: &mut Resources) -> Result<CapSlot, CAllocError> {
        self.alloc_cap(resources)
    }

    unsafe fn cap_free(&mut self, cap: CapId, resources: &mut Resources) {
        self.cap_free(cap, resources)
    }
}
