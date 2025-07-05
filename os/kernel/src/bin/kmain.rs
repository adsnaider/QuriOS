#![cfg(not(test))]
#![no_std]
#![no_main]

use kernel::kinit;
use serial::sprintln;

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    kinit();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // TODO: Reboot
    sprintln!("{}", info);
    loop {}
}
