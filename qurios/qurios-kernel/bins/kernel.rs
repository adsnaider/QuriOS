#![no_std]
#![no_main]

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    #[cfg(feature = "qemu")]
    qurios_qemu::init();

    kernel::arch::main();
}
