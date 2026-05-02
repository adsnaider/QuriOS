#![cfg(target_arch = "riscv64")]

pub fn main() -> ! {
    loop {}
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    loop {}
}
