#![cfg(not(test))]
#![no_std]
#![no_main]

use kernel::{kinit, uinit};
use serial::sprintln;

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    kinit();
    uinit();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // TODO: Reboot
    sprintln!("{}", info);
    loop {}
}
