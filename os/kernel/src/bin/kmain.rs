#![cfg(not(test))]
#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    todo!();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use serial::sprintln;
    // TODO: Reboot
    sprintln!("{}", info);
    loop {}
}
