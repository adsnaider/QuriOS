mod new_cap_macro;

pub mod ctable;
pub mod thread;
pub mod vmtable;

pub mod slotid;
pub use slotid::SysSlot;

mod cap;
pub use cap::{CapError, CapId, CapResult, PositiveIsize};
