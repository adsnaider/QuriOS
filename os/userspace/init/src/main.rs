#![cfg(not(test))]
#![no_std]
#![no_main]

use entry::entry;
use qapi::init::BootArgs;
use serial::sprintln;

#[entry]
fn main(_args: &'static BootArgs) -> ! {
    serial::init();
    log::info!("Landed on userspace init");
    todo!();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    sprintln!("{}", info);
    loop {}
}
