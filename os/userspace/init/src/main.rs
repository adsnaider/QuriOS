#![no_std]
#![no_main]

use entry::entry;
use qapi::{
    caps::{
        SlotId,
        cap_table::{ConsArgs, ThreadCons},
    },
    init::{BootArgs, BootCaps},
    mem::Frame,
};

#[entry]
fn main(args: &'static BootArgs) -> ! {
    serial::init();
    let bootcaps = BootCaps::new();
    let memory_map = args.memory_map.into_slice();
    let frame = memory_map
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.0.load(core::sync::atomic::Ordering::Relaxed) == 0x4000)
        .map(|(idx, _)| Frame::from_index(idx))
        .unwrap();
    log::info!("Found unused frame: {frame:?}");
    frame.retype(qapi::mem::RetypeKind::IntoKernel).unwrap();
    let mut t2_stack = [0usize; 128];
    bootcaps
        .self_caps
        .construct(
            ConsArgs::Thread(ThreadCons {
                entry: thread2 as usize,
                rsp: t2_stack.as_mut_slice().as_mut_ptr() as usize,
                addrspace: bootcaps.self_addrspace,
                caps: bootcaps.self_caps,
                frame: frame.base(),
            }),
            SlotId::new(10).unwrap(),
        )
        .expect("Unable to construct second thread");
    log::info!("Landed on userspace init");
    todo!();
}

fn thread2() -> ! {
    log::info!("Landed on userspace init");
    #[allow(clippy::empty_loop)]
    loop {}
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use serial::sprintln;
    sprintln!("{}", info);
    loop {}
}
