#![cfg(not(test))]
#![no_std]
#![no_main]

use kernel::{kinit, uinit};

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    kinit();
    let dispatcher = uinit();
    dispatcher.dispatch();
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use kernel::arch::backtrace::Backtrace;
    // TODO: Reboot
    use serial::sprintln;
    let backtrace = Backtrace::capture();
    sprintln!("{}", info);
    sprintln!("{}", backtrace);
    loop {}
}
