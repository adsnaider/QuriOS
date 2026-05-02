#![no_std]
#![no_main]

#[cfg_attr(target_arch = "riscv64", path = "riscv64/mod.rs")]
#[cfg_attr(target_arch = "aarch64", path = "aarch64/mod.rs")]
#[cfg_attr(target_arch = "x86_64", path = "x86_64/mod.rs")]
pub mod arch;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    arch::main();
}
