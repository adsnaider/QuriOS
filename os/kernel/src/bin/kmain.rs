#![cfg(not(test))]
#![no_std]
#![no_main]

use core::{
    arch::asm,
    fmt::{Display, write},
};

use kernel::{kinit, uinit};

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    kinit();
    let dispatcher = uinit();
    dispatcher.dispatch();
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // TODO: Reboot
    use serial::sprintln;
    let backtrace = Backtrace::capture();
    sprintln!("{}", info);
    sprintln!("{}", backtrace);
    loop {}
}

#[repr(C)]
struct Stackframe {
    rbp: *mut Stackframe,
    rip: *mut (),
}

pub struct Backtrace(*mut Stackframe);

impl Backtrace {
    pub fn capture() -> Self {
        let rbp: *mut Stackframe;
        unsafe {
            asm!("mov {}, rbp", out(reg) rbp);
        }
        Self(rbp)
    }
}

impl Display for Backtrace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        const MAX_FRAMES: usize = 1024;
        writeln!(f, "======== BACKTRACE ========")?;
        let mut frame = self.0;
        for _ in 0..MAX_FRAMES {
            if frame.is_null() {
                break;
            }
            writeln!(f, "{:#?}", unsafe { (*frame).rip });
            frame = unsafe { (*frame).rbp } as *mut Stackframe;
        }
        writeln!(f, "======== END BACKTRACE ========")?;

        Ok(())
    }
}
