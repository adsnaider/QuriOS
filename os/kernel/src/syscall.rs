use arch::{ArchSystem, System};
use ghost_cell::GhostToken;
use qapi::{
    caps::{CapError, PositiveIsize},
    syscall::{SyscallArgs, SyscallArgsInit},
};
use syscall_token::SyscallToken;
use tap::Tap;

use crate::{core_local::CoreLocalData, thread::Thread};

pub mod syscall_token {
    use ghost_cell::GhostToken;

    pub struct SyscallToken<'syscall>(GhostToken<'syscall>);

    impl<'a> SyscallToken<'a> {
        pub const unsafe fn new(token: GhostToken<'a>) -> Self {
            Self(token)
        }
    }
}

pub fn syscall_handler(
    cap: usize,
    args: SyscallArgs<SyscallArgsInit>,
    ctx: <ArchSystem as System>::SyscallCtx,
) -> Result<PositiveIsize, CapError> {
    GhostToken::new(|token| {
        // SAFETY: This is by definition the only okay place to construct this.
        let mut token = unsafe { SyscallToken::new(token) };
        let cap = cap.try_into()?;
        // SAFETY: We are allowed to get a mutable reference at the start of the syscall. It will get dropped
        let core_data = CoreLocalData::get_mut(&mut token);
        log::info!("Handling syscall: {cap} with args {args:?} {ctx:#?}");
        let current = &core_data.current_thread;
        // SAFETY: We should always have a thread set on syscall handling
        let cap = unsafe {
            current
                .as_ref()
                .tap(|t| debug_assert!(t.is_some()))
                .unwrap_unchecked()
                .active_comp()
                .cap(cap)
        }
        .ok_or(CapError::CapNotFound)?;
        cap.exercise(args)
    })
}
