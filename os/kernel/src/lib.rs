#![no_std]

use limine::{
    memory_map::Entry,
    request::{MemoryMapRequest, StackSizeRequest},
    BaseRevision,
};
use sync::singleton::Singleton;

static BASE_REVISION: BaseRevision = BaseRevision::new();
static MEMORY_MAP: Singleton<MemoryMapRequest> = Singleton::new(MemoryMapRequest::new());
static STACK_SIZE: StackSizeRequest = StackSizeRequest::new().with_size(0x32000);

pub type MemoryMap = &'static mut [&'static mut Entry];

pub fn kinit() -> ! {
    serial::init();
    assert!(BASE_REVISION.is_supported());
    STACK_SIZE
        .get_response()
        .expect("Limine stack size response missing");
    let mut memory_map = MEMORY_MAP
        .take()
        .expect("Memory map taken pre-intialization");
    let _memory_map = memory_map
        .get_response_mut()
        .expect("Missing memory map reponse")
        .entries_mut();
    let _sys = arch::init();
    todo!();
}
