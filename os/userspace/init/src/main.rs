#![cfg(not(test))]
#![no_std]
#![no_main]

use entry::entry;
use qapi::init::BootArgs;

#[entry]
fn main(_args: &'static BootArgs) -> ! {
    // SAFETY: Oh well... Just for testing page faults.
    unsafe {
        core::ptr::read_volatile(0xDEADFEE as *const u8);
    }
    todo!();
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
