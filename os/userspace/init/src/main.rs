#![no_std]
#![no_main]

use entry::entry;
use qapi::{
    caps::{
        SlotId,
        cap_table::{ConsArgs, ThreadCons},
    },
    init::{BootArgs, BootCaps},
};
use serial::sprintln;

#[entry]
fn main(_args: &'static BootArgs) -> ! {
    serial::init();
    let bootcaps = BootCaps::new();
    let mut t2_stack = [0usize; 128];
    bootcaps
        .self_caps
        .construct(
            ConsArgs::Thread(ThreadCons {
                entry: thread2 as usize,
                rsp: t2_stack.as_mut_slice().as_mut_ptr() as usize,
                addrspace: bootcaps.self_addrspace,
                caps: bootcaps.self_caps,
            }),
            SlotId::new(10).unwrap(),
        )
        .expect("Unable to construct second thread");
    log::info!("Landed on userspace init");
    todo!();
}

fn thread2() -> ! {
    log::info!("Landed on userspace init");
    loop {}
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    sprintln!("{}", info);
    loop {}
}
