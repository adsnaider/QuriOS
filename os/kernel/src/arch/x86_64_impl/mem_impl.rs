pub mod page_table;

use crate::arch::mem::{Frame, Page};

impl Page {
    pub const SIZE: usize = 4096;
}
impl Frame {
    pub const SIZE: u64 = 4096;
}
