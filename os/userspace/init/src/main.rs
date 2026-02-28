#![no_std]
#![no_main]

use core::alloc::GlobalAlloc;
use core::mem::MaybeUninit;
use core::ptr::NonNull;

use allocator_api2::boxed::Box;
use entry::entry;
use loader::MagicInfo;
use qapi::caps::notify::NotificationCap;
use qapi::caps::slotid::SlotId;
use qapi::caps::sync_ipc::{ExceptionAbi, StandardAbi, SyncInvokeCap};
use qapi::caps::thread::ThreadCap;
use qapi::caps::{CapId, PositiveIsize};
use qapi::exception::ExceptionInfo;
use qapi::init::{BootArgs, BootCaps, EXCEPTION_HANDLER_MAGIC};
use qapi::syscall::ops::retype::RetypeKind;
use serial::sprint;
use stack_list::StackNode;
use static_cell::{ConstStaticCell, StaticCell};
use sync::cell::AtomicOnceCell;
use ulib::allocation::allocman::{Allocman, Resources, SharedResources};
use ulib::allocation::cspace::CapabilityMan;
use ulib::allocation::pmspace::bitmap_allocator::BitmapAllocator;
use ulib::allocation::vmspace::Addrspace;
use ulib::sysops::sync_endpoint::{IPC_STACKS, SyncEndpoint};
use ulib::sysops::{CTableCapExt, FrameExt, IrqCtrlCapExt, SyncInvokeCapExt, ThreadCapExt};
use x86_64::instructions::port::Port;

static ALLOCATOR: spin::Mutex<Option<Allocman>> = spin::Mutex::new(None);

struct AllocmanGAlloc;
#[global_allocator]
static GALLOC: AllocmanGAlloc = AllocmanGAlloc;

unsafe impl GlobalAlloc for AllocmanGAlloc {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        ALLOCATOR
            .lock()
            .as_mut()
            .unwrap()
            .mem_alloc(layout)
            .unwrap()
            .as_ptr() as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        // SAFETY: Precondition and ptr must have been allocated with this allocator so it can't be null
        unsafe {
            ALLOCATOR
                .lock()
                .as_mut()
                .unwrap()
                .mem_free(NonNull::new_unchecked(ptr as _), layout)
        }
    }
}

#[used]
static EXCEPTION_HANDLER: MagicInfo<extern "C" fn()> = const {
    let endpoint = SyncEndpoint::<1, _, _>::create(ExceptionAbi, |args| {
        exception_handler(args.try_into().unwrap())
    });
    MagicInfo::new(EXCEPTION_HANDLER_MAGIC, endpoint.stackfull_endpoint())
};

#[entry]
fn main(args: &'static BootArgs) -> ! {
    // Aparently #[used] is not good enough...
    EXCEPTION_HANDLER.keep();
    #[allow(static_mut_refs)]
    {
        static mut SNODE1: [u128; 4096] = [0; 4096];
        static mut SEXCEPT1: [u128; 128] = [0; 128];
        // SAFETY: Only used as part of the IPC stacks.
        IPC_STACKS[0].push_front(StackNode::new(unsafe { &mut SNODE1 }).unwrap());
        // SAFETY: Only used as part of the IPC stacks.
        IPC_STACKS[1].push_front(StackNode::new(unsafe { &mut SEXCEPT1 }).unwrap());
    }
    serial::init();
    log::info!("Landed on userspace init");
    let bootcaps = BootCaps::new();
    let mut falloc = BitmapAllocator::new(args.memory_map.as_slice());

    bootcaps
        .self_caps
        .make_sync_call(
            SlotId::try_new(10).unwrap(),
            SyncEndpoint::<0, _, _>::create(StandardAbi, |(a, b, c, d)| sync_invoke(a, b, c, d))
                .stackfull_endpoint(),
            bootcaps.self_resources,
        )
        .unwrap();

    let sync_cap = SyncInvokeCap::new(CapId::new(10));
    let result = sync_cap
        .invoke([
            MaybeUninit::new(11),
            MaybeUninit::new(12),
            MaybeUninit::new(13),
            MaybeUninit::new(14),
        ])
        .unwrap();
    log::info!("Sync response: {result}");

    let free_frame = serial::sdbg!(falloc.alloc().unwrap());
    free_frame.retype(RetypeKind::IntoKernel).unwrap();
    let mut irq_stack: [u128; 128] = [0; 128];
    bootcaps
        .self_caps
        .make_thread(
            SlotId::try_new(11).unwrap(),
            irq_handler,
            irq_stack.as_mut_ptr() as *mut (),
            bootcaps.self_resources,
            free_frame,
            bootcaps.irq_ctrl.cap().into(),
            u32::MAX,
            bootcaps.self_thread,
        )
        .unwrap();

    // let irq_thread = ThreadCap::new(CapId::new(11));

    /*
    for i in 0..15 {
        bootcaps
            .self_caps
            .make_notification(
                SlotId::try_new(i + 12).unwrap(),
                ThreadCap::new(CapId::new(11)),
                1 << i,
            )
            .unwrap();
        let irq_notification = NotificationCap::new(CapId::new(12 + i as u32));
        bootcaps.irq_ctrl.irq_set(irq_notification, i).unwrap();
    }
    */
    // irq_thread.dispatch().unwrap();

    {
        static FIXED_POOL: ConstStaticCell<[MaybeUninit<u8>; 4096]> =
            ConstStaticCell::new([const { MaybeUninit::uninit() }; 4096]);
        static RESOURCES: StaticCell<SharedResources> = StaticCell::new();
        let resources = &*RESOURCES.init(SharedResources::new(Resources::new(
            FIXED_POOL.take(),
            serial::sdbg!(
                args.free_space_start
                    .next_multiple_of(1 << 12 << 9 << 9 << 9) as *mut u8
            ),
        )));
        let allocman = Allocman::new(
            CapabilityMan::new_starting_at(bootcaps.self_caps, SlotId::new(30), resources),
            falloc,
            Addrspace::new(bootcaps.self_addrspace, resources),
            resources,
        );
        assert!(ALLOCATOR.lock().replace(allocman).is_none());
    }
    log::info!("Attempting to allocate");
    let foo = Box::new(10);
    assert_eq!(*foo, 10);
    #[allow(clippy::empty_loop)]
    loop {}
}

fn sync_invoke(a: usize, b: usize, c: usize, d: usize) -> PositiveIsize {
    log::info!("Synchronous call: ({a}, {b}, {c}, {d})");
    10isize.try_into().unwrap()
}

fn exception_handler(e: ExceptionInfo) {
    panic!("Exception: {e:?}");
}

extern "C" fn irq_handler(_: usize) -> ! {
    loop {
        let signals = ThreadCap::sig_wait().unwrap();
        if signals & 0b1 > 0 {
            sprint!(".");
        }
        if signals & 0b10 > 0 {
            let mut port = Port::new(0x60);
            // SAFETY: Only place accessing the port is here.
            let scancode: u8 = unsafe { port.read() };
            sprint!("{}", scancode);
        }
    }
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use serial::sprintln;
    sprintln!("{}", info);
    // TODO: Reboot or something...
    #[allow(clippy::empty_loop)]
    loop {}
}
