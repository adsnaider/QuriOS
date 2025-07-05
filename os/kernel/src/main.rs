#![no_std]
#![no_main]

#[cfg(all(target_os = "none", not(test)))]
#[no_mangle]
extern "C" fn kmain() -> ! {
    todo!();
}

#[cfg(all(target_os = "none", not(test)))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use serial::sprintln;
    // TODO: Reboot
    sprintln!("{}", info);
    loop {}
}
