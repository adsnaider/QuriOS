#![no_std]
#![no_main]

use core::arch::naked_asm;
use core::mem::MaybeUninit;

use allocator_api2::boxed::Box;
use derive_more::{Deref, DerefMut};
use entry::entry;
use loader::MagicInfo;
use qapi::caps::slotid::SlotId;
use qapi::caps::sync_ipc::{ExceptionAbi, StandardAbi, SyncInvokeCap};
use qapi::caps::{CapId, PositiveIsize};
use qapi::exception::{ExceptionInfo, ExceptionKind};
use qapi::init::{BootArgs, BootCaps, EXCEPTION_HANDLER_ID};
use qapi::syscall::SyscallOp;
use qapi::syscall::ops::sync_ipc::SyncCallFun;
use serial::sprintln;
use stack_list::{StackList, StackNode, stack_list_pop, stack_list_push};
use ulib::alloc::allocman::{ALockedMan, Allocman, ReservedHeap};
use ulib::alloc::caps::CapabilityMan;
use ulib::alloc::phys::bitmap_allocator::BitmapAllocator;
use ulib::alloc::virt::Addrspace;
use ulib::sysops::sync_endpoint::{IPC_STACKS, SyncEndpoint};
use ulib::sysops::{CTableCapExt, CapIdExt, SyncInvokeCapExt};

#[global_allocator]
static ALLOCATOR: ALockedMan<BitmapAllocator> = ALockedMan::uninit();

#[used]
static EXCEPTION_HANDLER: MagicInfo<extern "C" fn()> = const {
    let endpoint = SyncEndpoint::<1, _, _>::create(ExceptionAbi, |args| {
        exception_handler(args.try_into().unwrap())
    });
    MagicInfo::new(EXCEPTION_HANDLER_ID, endpoint.stackfull_endpoint())
};

#[entry]
fn main(args: &'static BootArgs) -> ! {
    // Aparently #[used] is not good enough...
    EXCEPTION_HANDLER.keep();
    {
        static mut SNODE1: [u128; 4096] = [0; 4096];
        static mut SEXCEPT1: [u128; 128] = [0; 128];
        #[allow(static_mut_refs)]
        IPC_STACKS[0].push_front(StackNode::new(unsafe { &mut SNODE1 }).unwrap());
        #[allow(static_mut_refs)]
        IPC_STACKS[1].push_front(StackNode::new(unsafe { &mut SEXCEPT1 }).unwrap());
    }
    serial::init();
    log::info!("Landed on userspace init");
    let bootcaps = BootCaps::new();
    bootcaps
        .self_caps
        .make_sync_call(
            SlotId::new(10).unwrap(),
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

    unsafe {
        core::ptr::read_volatile(0xA_F00_BABE as *const u8);
    }

    /*
    let cspace = CapabilityMan::new_starting_at(bootcaps.self_caps, BootCaps::next_free());

    for i in 0..BootCaps::next_free().as_usize() {
        log::info!(
            "Introspecting capability ({i}): {:#?}",
            CapId::new(i as u32).introspect()
        )
    }



    let falloc = BitmapAllocator::new(
        // bootcaps.self_addrspace,
        args.memory_map.as_slice(),
        // args.free_space_start,
    );
    let vspace = Addrspace::new(bootcaps.self_addrspace, ReservedHeap::empty());
    ALLOCATOR.set(Allocman::new(falloc, vspace, cspace));

    let foo = Box::new(10);
    assert_eq!(*foo, 10);
    */

    todo!();
}

fn sync_invoke(a: usize, b: usize, c: usize, d: usize) -> PositiveIsize {
    log::info!("Synchronous call: ({a}, {b}, {c}, {d})");
    10isize.try_into().unwrap()
}

fn exception_handler(e: ExceptionInfo) {
    panic!("Exception: {e:?}");
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use serial::sprintln;
    sprintln!("{}", info);
    loop {}
}
