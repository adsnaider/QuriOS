#![no_std]
#![no_main]

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    loop {}
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    loop {}
}
