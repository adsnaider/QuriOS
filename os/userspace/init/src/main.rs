#![no_std]
#![no_main]

use allocator_api2::boxed::Box;
use entry::entry;
use qapi::init::{BootArgs, BootCaps};
use ulib::alloc::allocman::{ALockedMan, Allocman, ReservedHeap};
use ulib::alloc::caps::CapabilityMan;
use ulib::alloc::phys::bitmap_allocator::BitmapAllocator;
use ulib::alloc::virt::Addrspace;

#[global_allocator]
static ALLOCATOR: ALockedMan<BitmapAllocator> = ALockedMan::uninit();

#[entry]
fn main(args: &'static BootArgs) -> ! {
    serial::init();
    log::info!("Landed on userspace init");
    let bootcaps = BootCaps::new();
    let cspace = CapabilityMan::new_starting_at(bootcaps.self_caps, BootCaps::next_free());
    let falloc = BitmapAllocator::new(
        // bootcaps.self_addrspace,
        args.memory_map.as_slice(),
        // args.free_space_start,
    );
    let vspace = Addrspace::new(bootcaps.self_addrspace, ReservedHeap::empty());
    ALLOCATOR.set(Allocman::new(falloc, vspace, cspace));

    let foo = Box::new(10);
    assert_eq!(*foo, 10);

    todo!();
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use serial::sprintln;
    sprintln!("{}", info);
    loop {}
}
