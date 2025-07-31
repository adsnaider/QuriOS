#![no_std]
#![no_main]

use entry::entry;
use qapi::{
    caps::{CapId, SlotId, thread::Thread},
    init::{BootArgs, BootCaps},
    mem::Frame,
};

#[entry]
fn main(args: &'static BootArgs) -> ! {
    serial::init();
    log::info!("Landed on userspace init");
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
        .make_thread(
            SlotId::new(10).unwrap(),
            thread2,
            // SAFETY: byte_sub doesn't exceed past the range of the slice
            unsafe { t2_stack.as_mut_slice().as_mut_ptr_range().end.byte_sub(8) as *mut () },
            bootcaps.self_addrspace,
            bootcaps.self_caps,
            frame,
            42,
        )
        .expect("Unable to construct second thread");
    let thread = Thread::new(CapId::new(10));
    log::info!("Jumping to thread 2");
    thread.dispatch().expect("Couldn't dispatch second thread");
    log::info!("Back from thread 2");
    todo!();
}

extern "C" fn thread2(arg0: usize) -> ! {
    log::info!("Landed on userspace thread 2: {arg0}");
    let thread = Thread::new(CapId::new(2));
    thread.dispatch().expect("Error jumping to thread 1");
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
