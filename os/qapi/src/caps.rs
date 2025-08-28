mod new_cap_macro;

pub mod ctable;
pub mod notify;
pub mod resources;
pub mod sync_ipc;
pub mod thread;
pub mod vmtable;

pub mod slotid;
pub use slotid::SysSlot;

mod cap;
pub use cap::{CapError, CapId, CapResult, PositiveIsize};
