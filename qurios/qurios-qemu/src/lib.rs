//! Helpers to communicate qemu-specific hardwware
#![no_std]
#![feature(const_trait_impl)]
#![feature(const_cmp)]

mod serial;

pub fn init() {
    cfg_select! {
        target_arch = "x86_64" => {
            serial::init();
        }
        target_arch = "riscv64" => {
            let serial = serial::SerialPort::new(0xFFFF_8000_0000_0000);
            serial::init_with(serial);
        }
    }
}
