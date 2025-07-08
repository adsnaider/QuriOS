mod asm_utils;
use asm_utils::{pop_scratch, push_scratch};

use core::arch::naked_asm;

use x86_64::structures::idt::{
    DivergingHandlerFunc, DivergingHandlerFuncWithErrCode, Entry, EntryOptions, HandlerFunc,
    HandlerFuncWithErrCode, InterruptStackFrame, PageFaultHandlerFunc,
};

use crate::x86_64_impl::exec::{ExceptionCtx, Interrupt};

pub trait Isr: Sized {
    extern "sysv64" fn call(frame: &InterruptStackFrame);
    fn register_interrupt<F>(entry: &mut Entry<HandlerFunc>, handler: F) -> &mut EntryOptions
    where
        F: Fn(ExceptionCtx<Interrupt>) + Zst,
    {
        F::verify_zst();
        // SAFETY: Handler matches the expected handler kind (error code vs not)
        unsafe { entry.set_handler_addr(x86_64::VirtAddr::from_ptr(isr::<F> as *const ())) }
    }
}

pub trait DivergingIsr: Sized {
    extern "sysv64" fn call(frame: &InterruptStackFrame) -> !;
    fn register(entry: &mut Entry<DivergingHandlerFunc>, _isr: Self) -> &mut EntryOptions {
        // SAFETY: Handler matches the expected handler kind (error code vs not)
        unsafe {
            entry.set_handler_addr(x86_64::VirtAddr::from_ptr(
                diverging_isr::<Self> as *const (),
            ))
        }
    }
}

pub trait ErrCodeIsr: Sized {
    extern "sysv64" fn call(frame: &InterruptStackFrame, code: u64);

    fn register(entry: &mut Entry<HandlerFuncWithErrCode>, _isr: Self) -> &mut EntryOptions {
        // SAFETY: Handler matches the expected handler kind (error code vs not)
        unsafe {
            entry.set_handler_addr(x86_64::VirtAddr::from_ptr(
                isr_with_err_code::<Self> as *const (),
            ))
        }
    }
    fn register_page_fault(
        entry: &mut Entry<PageFaultHandlerFunc>,
        _isr: Self,
    ) -> &mut EntryOptions {
        // SAFETY: Handler matches the expected handler kind (error code vs not)
        unsafe {
            entry.set_handler_addr(x86_64::VirtAddr::from_ptr(
                isr_with_err_code::<Self> as *const (),
            ))
        }
    }
}

pub trait DivergingErrCodeIsr: Sized {
    extern "sysv64" fn call(frame: &InterruptStackFrame, code: u64) -> !;
    fn register(
        entry: &mut Entry<DivergingHandlerFuncWithErrCode>,
        _isr: Self,
    ) -> &mut EntryOptions {
        // SAFETY: Handler matches the expected handler kind (error code vs not)
        unsafe {
            entry.set_handler_addr(x86_64::VirtAddr::from_ptr(
                diverging_isr_with_err_code::<Self> as *const (),
            ))
        }
    }
}

// SAFETY: Not actually extern "C". This is more of a x86_interrupt abi
#[unsafe(naked)]
extern "C" fn isr<F: Fn(ExceptionCtx<Interrupt>)>() {
    #[allow(unused_unsafe)]
    // SAFETY: Sticking with ISR calling convention. Preserved registers are pushed on
    // call to sysv64 ABI.
    unsafe {
        naked_asm!(push_scratch!(), "lea rdi, [rsp + 8*9]", "call {inner}", pop_scratch!(), "iretq", inner = sym I::call);
    }
}

#[unsafe(naked)]
extern "C" fn diverging_isr<I: DivergingIsr>() {
    #[allow(unused_unsafe)]
    // SAFETY: Sticking with ISR calling convention. Preserved registers are pushed on
    // call to sysv64 ABI.
    unsafe {
        naked_asm!("sub rsp, 8", "lea rdi, [rsp + 8]", "call {inner}", "ud2", inner = sym I::call);
    }
}

#[unsafe(naked)]
extern "C" fn isr_with_err_code<I: ErrCodeIsr>() {
    #[allow(unused_unsafe)]
    // SAFETY: Sticking with ISR calling convention. Preserved registers are pushed on
    // call to sysv64 ABI.
    unsafe {
        naked_asm!(push_scratch!(), "sub rsp, 8", "lea rdi, [rsp + 8*11]", "mov rsi, [rsp + 8*10]",  "call {inner}", pop_scratch!(), "iretq", inner = sym I::call);
    }
}

#[unsafe(naked)]
extern "C" fn diverging_isr_with_err_code<I: DivergingErrCodeIsr>() {
    #[allow(unused_unsafe)]
    // SAFETY: Sticking with ISR calling convention. Preserved registers are pushed on
    // call to sysv64 ABI.
    unsafe {
        naked_asm!("lea rdi, [rsp]", "call {inner}", "ud2", inner = sym I::call);
    }
}

pub struct PanicHandler<const ID: usize>;
impl<ID: usize> Fn(ExceptionCtx<Interrupt>) for PanicHandler<ID> {
    extern "rust-call" fn call(&self, args: Args) -> Self::Output {
        todo!()
    }
}
impl<const ID: usize> Isr for PanicHandler<ID> {
    extern "sysv64" fn call(frame: &InterruptStackFrame) {
        <Self as DivergingIsr>::call(frame)
    }
}
impl<const ID: usize> DivergingIsr for PanicHandler<ID> {
    extern "sysv64" fn call(frame: &InterruptStackFrame) -> ! {
        panic!("The requested interrupt (ID: {ID}) is not yet implemented\n{frame:#?}");
    }
}
impl<const ID: usize> ErrCodeIsr for PanicHandler<ID> {
    extern "sysv64" fn call(frame: &InterruptStackFrame, error: u64) {
        <Self as DivergingErrCodeIsr>::call(frame, error)
    }
}
impl<const ID: usize> DivergingErrCodeIsr for PanicHandler<ID> {
    extern "sysv64" fn call(frame: &InterruptStackFrame, error: u64) -> ! {
        panic!(
            "The requested interrupt (ID: {ID}) (code: {error}) is not yet implemented\n{frame:#?}"
        );
    }
}

impl<F: Fn()> Isr for F
where
    F: Zst,
{
    extern "sysv64" fn call(_frame: &InterruptStackFrame) {
        F::verify_zst();
        let f: *const Self = core::ptr::dangling();
        // SAFETY: Non-zero dangling pointer is okay as it's a compile-time only artifact.
        unsafe {
            (*f)();
        }
    }
}

trait Zst: Sized {
    const ZST: () = const { assert!(core::mem::size_of::<Self>() == 0, "Type is not zero-sized") };
    fn verify_zst() {
        let () = Self::ZST;
        debug_assert!(core::mem::size_of::<Self>() == 0);
    }
}

impl<T> Zst for T {}
