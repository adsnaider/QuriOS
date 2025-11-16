use core::arch::asm;
use core::fmt::Display;

#[repr(C)]
struct Stackframe {
    rbp: *mut Stackframe,
    rip: *mut (),
}

pub struct Backtrace(*mut Stackframe);

impl Backtrace {
    pub fn capture() -> Self {
        let rbp: *mut Stackframe;
        // SAFETY: Just reading `rbp` register out
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
            // SAFETY: We can reasonably expect the `rbp` to be pointing to the stack frame
            let stackframe = unsafe { &*frame };
            writeln!(f, "{:#?}", stackframe.rip)?;
            // SAFETY: We can reasonably expect the `rbp` to be pointing to the stack frame
            frame = stackframe.rbp;
        }
        writeln!(f, "======== END BACKTRACE ========")?;

        Ok(())
    }
}
