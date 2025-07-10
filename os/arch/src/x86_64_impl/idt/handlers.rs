mod asm_utils;
use core::{arch::naked_asm, convert::Infallible};

use asm_utils::{pop_scratch, push_scratch};
use sealed::sealed;
use x86_64::structures::idt::{
    DivergingHandlerFunc, DivergingHandlerFuncWithErrCode, Entry, EntryOptions, HandlerFunc,
    HandlerFuncWithErrCode, PageFaultHandlerFunc,
};

use crate::x86_64_impl::exec::{Exception, ExceptionCtx, Interrupt};

pub trait IsrHandler<Kind, Ret> {
    extern "sysv64" fn call(_ctx: ExceptionCtx<Kind>) -> Ret {
        unimplemented!();
    }
}

pub unsafe trait Isr: Sized {
    type Kind;
    type Ret;

    fn register<H>(&mut self, handler: H) -> &mut EntryOptions
    where
        H: IsrHandler<Self::Kind, Self::Ret> + Zst;
}

unsafe impl Isr for Entry<HandlerFunc> {
    type Kind = Interrupt;
    type Ret = ();

    fn register<H>(&mut self, _handler: H) -> &mut EntryOptions
    where
        H: IsrHandler<Self::Kind, Self::Ret> + Zst,
    {
        H::verify_zst();
        unsafe {
            self.set_handler_addr(x86_64::VirtAddr::from_ptr(
                save_all_and_ret_isr::<H> as *const (),
            ))
        }
    }
}

unsafe impl Isr for Entry<HandlerFuncWithErrCode> {
    type Kind = Exception;
    type Ret = ();

    fn register<H>(&mut self, _handler: H) -> &mut EntryOptions
    where
        H: IsrHandler<Self::Kind, Self::Ret> + Zst,
    {
        H::verify_zst();
        unsafe {
            self.set_handler_addr(x86_64::VirtAddr::from_ptr(
                save_all_with_err_code_and_ret_isr::<H> as *const (),
            ))
        }
    }
}

unsafe impl Isr for Entry<PageFaultHandlerFunc> {
    type Kind = Exception;
    type Ret = ();

    fn register<H>(&mut self, _handler: H) -> &mut EntryOptions
    where
        H: IsrHandler<Self::Kind, Self::Ret> + Zst,
    {
        H::verify_zst();
        unsafe {
            self.set_handler_addr(x86_64::VirtAddr::from_ptr(
                save_all_with_err_code_and_ret_isr::<H> as *const (),
            ))
        }
    }
}

unsafe impl Isr for Entry<DivergingHandlerFunc> {
    type Kind = Interrupt;
    type Ret = Infallible;

    fn register<H>(&mut self, _handler: H) -> &mut EntryOptions
    where
        H: IsrHandler<Self::Kind, Self::Ret> + Zst,
    {
        H::verify_zst();
        unsafe {
            self.set_handler_addr(x86_64::VirtAddr::from_ptr(
                save_some_and_diverge_isr::<H> as *const (),
            ))
        }
    }
}

unsafe impl Isr for Entry<DivergingHandlerFuncWithErrCode> {
    type Kind = Exception;
    type Ret = Infallible;

    fn register<H>(&mut self, _handler: H) -> &mut EntryOptions
    where
        H: IsrHandler<Self::Kind, Self::Ret> + Zst,
    {
        H::verify_zst();
        unsafe {
            self.set_handler_addr(x86_64::VirtAddr::from_ptr(
                save_some_with_err_code_and_diverge_isr::<H> as *const (),
            ))
        }
    }
}

impl<F, Kind, Ret> IsrHandler<Kind, Ret> for F
where
    F: Fn(ExceptionCtx<Kind>) -> Ret,
{
    extern "sysv64" fn call(_ctx: ExceptionCtx<Kind>) -> Ret {
        core::unimplemented!();
    }
}

pub struct PanicHandler<const ID: usize>;
impl<const ID: usize, Kind, Ret> IsrHandler<Kind, Ret> for PanicHandler<ID> {
    extern "sysv64" fn call(ctx: ExceptionCtx<Kind>) -> Ret {
        let interrupt_stack = ctx.interrupt_stack_frame();
        panic!("Unhandled exception ({ID}) {interrupt_stack:#?}");
    }
}

// SAFETY: Not actually extern "C". This is more of a x86_interrupt abi
#[unsafe(naked)]
extern "C" fn save_all_and_ret_isr<F: IsrHandler<Interrupt, ()>>() {
    #[allow(unused_unsafe)]
    // SAFETY: Sticking with ISR calling convention. Preserved registers are pushed on
    // call to sysv64 ABI.
    unsafe {
        naked_asm!(push_scratch!(), "lea rdi, [rsp + 8*9]", "call {inner}", pop_scratch!(), "iretq", inner = sym F::call);
    }
}

// SAFETY: Not actually extern "C". This is more of a x86_interrupt abi
#[unsafe(naked)]
extern "C" fn save_all_with_err_code_and_ret_isr<F: IsrHandler<Exception, ()>>() {
    #[allow(unused_unsafe)]
    // SAFETY: Sticking with ISR calling convention. Preserved registers are pushed on
    // call to sysv64 ABI.
    unsafe {
        naked_asm!(push_scratch!(), "sub rsp, 8", "lea rdi, [rsp + 8*11]", "call {inner}", "add rsp, 8", pop_scratch!(), "iretq", inner = sym F::call);
    }
}

// SAFETY: Not actually extern "C". This is more of a x86_interrupt abi
#[unsafe(naked)]
extern "C" fn save_some_and_diverge_isr<F: IsrHandler<Interrupt, Infallible>>() {
    #[allow(unused_unsafe)]
    // SAFETY: Sticking with ISR calling convention. Preserved registers are pushed on
    // call to sysv64 ABI.
    unsafe {
        naked_asm!("sub rsp, 8", "lea rdi, [rsp + 8]", "call {inner}", "ud2", inner = sym F::call);
    }
}

// SAFETY: Not actually extern "C". This is more of a x86_interrupt abi
#[unsafe(naked)]
extern "C" fn save_some_with_err_code_and_diverge_isr<F: IsrHandler<Exception, Infallible>>() {
    #[allow(unused_unsafe)]
    // SAFETY: Sticking with ISR calling convention. Preserved registers are pushed on
    // call to sysv64 ABI.
    unsafe {
        naked_asm!("lea rdi, [rsp]", "call {inner}", "ud2", inner = sym F::call);
    }
}

/*
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
*/

pub trait Zst: Sized {
    const ZST: () = const { assert!(core::mem::size_of::<Self>() == 0, "Type is not zero-sized") };
    fn verify_zst() {
        let () = Self::ZST;
        debug_assert!(core::mem::size_of::<Self>() == 0);
    }
}

impl<T> Zst for T {}
