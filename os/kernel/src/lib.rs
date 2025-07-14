#![no_std]

pub mod caps;
pub mod kmem;
pub mod pmo;
pub mod retyping;
pub mod syscall;
pub mod thread;

pub(crate) mod core_local;
pub(crate) mod hint;

mod boot;

use core::mem::ManuallyDrop;

use arch::{
    mem::{Pmo, VirtAddr},
    system, ArchSystem,
};
use boot::{bump_alloc::BumpFrameAllocator, Process};
use caps::Resources;
use kmem::KPtr;
use limine::{
    request::{HhdmRequest, MemoryMapRequest, ModuleRequest, StackSizeRequest},
    BaseRevision,
};
use sync::{cell::AtomicLazyCell, singleton::Singleton};
use syscall::syscall_handler;
use tap::TapFallible;
use tar_no_std::TarArchiveRef;
use thread::Thread;

static BASE_REVISION: BaseRevision = BaseRevision::new();
static MEMORY_MAP: Singleton<MemoryMapRequest> = Singleton::new(MemoryMapRequest::new());
static STACK_SIZE: StackSizeRequest = StackSizeRequest::new().with_size(0x32000);
static PMO: AtomicLazyCell<Pmo> = AtomicLazyCell::new(|| {
    static HHDM: HhdmRequest = HhdmRequest::new();
    let pmo = HHDM
        .get_response()
        .expect("Missing Higher-half direct mapping response from limine")
        .offset();
    // PMO must be on the higher half
    assert!(pmo >= 0xFFFF_8000_0000_0000);
    // SAFETY: PMO is provided by limine which can be trusted
    unsafe { Pmo::new(VirtAddr::new(pmo as usize)) }
});
static MODULES_REQUEST: ModuleRequest = ModuleRequest::new();
pub const UNTYPED_MEMORY_OFFSET: usize = 0x0000_7000_0000_0000;

pub fn kinit() {
    serial::init();
    assert!(BASE_REVISION.is_supported());
    STACK_SIZE
        .get_response()
        .expect("Limine stack size response missing");
    // SAFETY: PMO is correct from limine
    arch::init(*PMO.get(), syscall_handler);

    let memory_map: &'static mut MemoryMapRequest = MEMORY_MAP
        .take_ref_mut()
        .expect("Memory map taken pre-intialization");
    let memory_map = memory_map
        .get_response_mut()
        .expect("Missing memory map reponse")
        .entries_mut();
    // SAFETY: Memory map can be trusted to be correct
    unsafe { retyping::init(memory_map).expect("Error initializing retype table") }
}

/// Initializes the `init` userspace process
pub fn uinit() -> ! {
    let modules = MODULES_REQUEST.get_response().unwrap().modules();
    let initrd = modules
        .iter()
        .find(|module| module.path().to_bytes().ends_with(b"initrd.tar"))
        .expect("Bootloader didn't provide the initrd image");
    // SAFETY: The limine module is an in-memory file with the expected address and length and should be
    // valid until bootloader reclaimable data is used.
    let initrd =
        unsafe { core::slice::from_raw_parts(initrd.addr(), initrd.size().try_into().unwrap()) };
    log::info!("Loaded init image");

    let archive = TarArchiveRef::new(initrd).expect("Invalid initrd image");
    let proc = archive
        .entries()
        .filter_map(|e| {
            e.filename()
                .as_str()
                .tap_err(|err| log::warn!("Invalid entry in initrd: {err}. Skipping..."))
                .map(|_| e)
                .ok()
        })
        .find(|e| e.filename().as_str().unwrap() == "init")
        .expect("Missing init process from initrd")
        .data();

    log::info!("Found init image. Loading userspace process");
    let mut fallocator = BumpFrameAllocator::new();
    let init = Process::<ArchSystem>::load(system(), proc, 10, initrd, &mut fallocator)
        .expect("Error loading init process");
    let resources = Resources::new(init.addrspace);
    let thread = Thread::new(init.exec, resources);
    let thread_frame = fallocator
        .alloc_kernel_frame()
        .expect("Out of memory error during initialization");
    // SAFETY: The kernel frame is unused
    let thread = unsafe { KPtr::new_unchecked(thread_frame, ManuallyDrop::new(thread)) };
    Thread::dispatch(thread)
}
