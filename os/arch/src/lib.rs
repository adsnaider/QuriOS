#![no_std]

#[cfg(target_arch = "x86_64")]
mod x86_64_impl;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        pub fn init() -> impl System {
            x86_64_impl::Sys::init()
        }
    } else {
        const _: () = const { panic!("Target architecture not supported") };
    }
}

pub trait System {}

pub mod mem;
