#![cfg(not(test))]
#![no_std]
#![no_main]

use kernel::{kinit, uinit};

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    kinit();
    uinit();
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // TODO: Reboot
    use serial::sprintln;
    sprintln!("{}", info);
    loop {}
}
