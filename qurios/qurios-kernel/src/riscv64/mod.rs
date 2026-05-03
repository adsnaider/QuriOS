#![cfg(target_arch = "riscv64")]
#![allow(static_mut_refs)]

use limine::{
    BaseRevision,
    request::{FramebufferRequest, HhdmRequest, ModuleRequest, StackSizeRequest},
};

#[used]
static BASE_REVISION: BaseRevision = BaseRevision::new();
#[used]
static STACK_SIZE: StackSizeRequest = StackSizeRequest::new().with_size(0x32000);

#[used]
static HHDM: HhdmRequest = HhdmRequest::new();

#[used]
static MODULES_REQUEST: ModuleRequest = ModuleRequest::new();
#[used]
static mut FRAMEBUFFER: FramebufferRequest = FramebufferRequest::new();

pub fn main() -> ! {
    let pmo = HHDM.get_response().unwrap().offset() as usize;
    let framebuffer = unsafe { FRAMEBUFFER.get_response_mut().unwrap() };
    let fb = framebuffer.framebuffers().next().unwrap();
    for row in 0..fb.height() as usize {
        for col in 1..fb.width() as usize {
            let pixel_width: usize = fb.bpp() as usize / 8;
            unsafe {
                let pixel = fb.addr().add(row * fb.pitch() as usize + col * pixel_width);
                core::ptr::write_volatile(pixel, 0);
                core::ptr::write_volatile(pixel.add(1), 0xFF);
                core::ptr::write_volatile(pixel.add(2), 0x0);
            }
        }
    }
    log::info!("Hello from RiscV kernel");
    loop {}
}

#[allow(unused)]
#[cfg_attr(not(test), panic_handler)]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    let framebuffer = unsafe { FRAMEBUFFER.get_response_mut().unwrap() };
    let fb = framebuffer.framebuffers().next().unwrap();
    for row in 0..fb.height() as usize {
        for col in 1..fb.width() as usize {
            let pixel_width: usize = fb.bpp() as usize / 8;
            unsafe {
                let pixel = fb.addr().add(row * fb.pitch() as usize + col * pixel_width);
                core::ptr::write_volatile(pixel, 0xFF);
                core::ptr::write_volatile(pixel.add(1), 0x0);
                core::ptr::write_volatile(pixel.add(2), 0x0);
            }
        }
    }
    loop {}
}
