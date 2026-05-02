#![cfg(target_arch = "x86_64")]

pub fn main() -> ! {
    loop {}
}

#[allow(unused)]
#[cfg_attr(not(test), panic_handler)]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
