#![cfg(target_arch = "aarch64")]

pub fn main() -> ! {
    loop {}
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    loop {}
}
