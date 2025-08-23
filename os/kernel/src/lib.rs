#![no_std]

pub(crate) mod never;

pub mod arch;
pub mod caps;
pub mod kmem;
pub mod pmo;
pub mod retyping;
pub mod sync_call;
pub mod syscall;
pub mod thread;

pub(crate) mod core_local;
pub(crate) mod hint;

mod boot;

use core::sync::atomic::AtomicUsize;

use arch::{ArchCaps, System};
use boot::bump_alloc::BumpFrameAllocator;
use boot::Process;
use caps::trie::TrieSlotPayload;
use caps::{CapBlock, CapTable, Capability, Resources};
use core_local::CoreLocalDataKernelLocalStoreExt as _;
use kmem::KPtr;
use limine::request::{HhdmRequest, MemoryMapRequest, ModuleRequest, StackSizeRequest};
use limine::BaseRevision;
use qapi::caps::SysSlot;
use sync::cell::AtomicLazyCell;
use sync::singleton::Singleton;
use sync_call::SyncCall;
use tap::TapFallible;
use tar_no_std::TarArchiveRef;
use thread::Thread;

use crate::arch::mem::core_local::CoreLocalData;
use crate::arch::mem::{Pmo, VirtAddr};
use crate::arch::{system, ArchSystem};

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

const BOOTER_EXCEPTION_HANDLER_ADDR: usize = 0x1000;

pub fn kinit() {
    serial::init();
    assert!(BASE_REVISION.is_supported());
    STACK_SIZE
        .get_response()
        .expect("Limine stack size response missing");
    // SAFETY: PMO is correct from limine
    arch::init();

    let memory_map: &'static mut MemoryMapRequest = MEMORY_MAP
        .take_ref_mut()
        .expect("Memory map taken pre-intialization");
    let memory_map = memory_map
        .get_response_mut()
        .expect("Missing memory map reponse")
        .entries_mut();
    // SAFETY: Memory map can be trusted to be correct
    unsafe { retyping::init(memory_map).expect("Error initializing retype table") }
    system().post_init();
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

    CoreLocalData::init(
        fallocator
            .alloc_kernel_frame()
            .expect("Out of memory error during initialization"),
        0,
    );

    let init = Process::<ArchSystem>::load(system(), proc, 10, initrd, &mut fallocator)
        .expect("Error loading init process");

    let frame = fallocator
        .alloc_kernel_frame()
        .expect("Out of memory error during initialization");
    let cap_table = CapTable::default();
    // SAFETY: The kernel frame is unused
    let cap_table = unsafe { KPtr::new_unchecked(frame, cap_table) };
    let resources = Resources::new(
        init.addrspace,
        cap_table.clone(),
        caps::ExceptionHandler::Within {
            entry: BOOTER_EXCEPTION_HANDLER_ADDR,
        },
    );
    let resources_frame = fallocator
        .alloc_kernel_frame()
        .expect("Out of memory during initialization");
    // SAFETY: The kernel frame is unused
    let resources = unsafe { KPtr::new_unchecked(resources_frame, resources) };
    let thread = Thread::new(init.exec, resources.clone());
    let thread_frame = fallocator
        .alloc_kernel_frame()
        .expect("Out of memory error during initialization");
    // SAFETY: The kernel frame is unused
    let thread = unsafe { KPtr::new_unchecked(thread_frame, thread) };
    thread
        .set_affinity()
        .expect("Couldn't set init-thread affinity to core");

    CapBlock::at(
        // SAFETY: It's okay to cast a cap table to cap block.
        unsafe { thread.active_comp().unwrap().cap_table().cast_ref() },
        SysSlot::new(0).unwrap(),
    )
    .try_set(TrieSlotPayload::Data(
        Capability::<ArchSystem>::CompResource(resources),
    ))
    .expect("Unable to set boot capabilities");
    CapBlock::at(
        // SAFETY: It's okay to cast a cap table to cap block.
        unsafe { thread.active_comp().unwrap().cap_table().cast_ref() },
        SysSlot::new(1).unwrap(),
    )
    .try_set(TrieSlotPayload::Data(Capability::<ArchSystem>::CapBlock(
        // SAFETY: It's okay to cast a cap table to cap block.
        unsafe { cap_table.cast() },
    )))
    .expect("Unable to set boot capabilities");
    CapBlock::at(
        // SAFETY: It's okay to cast a cap table to cap block.
        unsafe { thread.active_comp().unwrap().cap_table().cast_ref() },
        SysSlot::new(2).unwrap(),
    )
    .try_set(TrieSlotPayload::Data(Capability::<ArchSystem>::Arch(
        <ArchSystem as System>::ArchCaps::new_addrspace(
            &*thread.active_comp().unwrap().addrspace(),
        ),
    )))
    .expect("Unable to set boot capabilities");
    CapBlock::at(
        // SAFETY: It's okay to cast a cap table to cap block.
        unsafe { thread.active_comp().unwrap().cap_table().cast_ref() },
        SysSlot::new(3).unwrap(),
    )
    .try_set(TrieSlotPayload::Data(Capability::<ArchSystem>::Thread(
        KPtr::clone(&thread),
    )))
    .expect("Unable to set boot capabilities");
    Thread::dispatch(thread, None).expect("Thread affinity was set above.");
}
