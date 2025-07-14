#![cfg(not(test))]
#![no_std]
#![no_main]

use entry::entry;
use qapi::{
    init::BootArgs,
    syscall::{SyscallArgs, ulib::syscall},
};
use serial::sprintln;

#[entry]
fn main(_args: &'static BootArgs) -> ! {
    serial::init();
    log::info!("Landed on userspace init");
    let result = syscall(SyscallArgs::uninit(1));
    log::info!("Syscall result: {result:?}");
    todo!();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    sprintln!("{}", info);
    loop {}
}
