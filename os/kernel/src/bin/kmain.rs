#![cfg(not(test))]
#![no_std]
#![no_main]

use serial::sprintln;

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    init();
    todo!();
}

fn init() {
    serial::init();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // TODO: Reboot
    sprintln!("{}", info);
    loop {}
}
