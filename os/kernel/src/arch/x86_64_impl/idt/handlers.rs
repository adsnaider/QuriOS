mod asm_utils;
use core::arch::naked_asm;

use asm_utils::{pop_preserved, pop_scratch, push_preserved, push_scratch};
use qapi::exception::ExceptionKind;
use x86_64::structures::idt::{
    DivergingHandlerFunc, DivergingHandlerFuncWithErrCode, Entry, EntryOptions, HandlerFunc,
    HandlerFuncWithErrCode, PageFaultHandlerFunc,
};

use crate::{
    arch::x86_64_impl::exec::{Exception, ExceptionCtx, Interrupt},
    never::Never,
};

pub trait IsrHandler<Kind, Ret> {
    extern "sysv64" fn call(_ctx: ExceptionCtx<Kind>) -> Ret;
}

/// Defines a type of ISR with a different ISR definition
pub trait Isr {
    /// Type of ISR (usually Exception (with error code) or Interrupt)
    type Kind;
    /// Return type for the ISR (usually void `()` or Diverging `!`)
    type Ret;

    /// Registers the handler for the ISR.
    fn register<H>(&mut self, handler: H) -> &mut EntryOptions
    where
        H: IsrHandler<Self::Kind, Self::Ret>;
}

impl Isr for Entry<HandlerFunc> {
    type Kind = Interrupt;
    type Ret = ();

    fn register<H>(&mut self, _handler: H) -> &mut EntryOptions
    where
        H: IsrHandler<Self::Kind, Self::Ret>,
    {
        // SAFETY: Handler address is valid for this type of ISR. We guarantee
        // that the ISR will match the expected interrupt stack
        unsafe {
            self.set_handler_addr(x86_64::VirtAddr::from_ptr(
                save_all_and_ret_isr::<H> as *const (),
            ))
        }
    }
}

impl Isr for Entry<HandlerFuncWithErrCode> {
    type Kind = Exception;
    type Ret = ();

    fn register<H>(&mut self, _handler: H) -> &mut EntryOptions
    where
        H: IsrHandler<Self::Kind, Self::Ret>,
    {
        // SAFETY: Handler address is valid for this type of ISR. We guarantee
        // that the ISR will match the expected interrupt stack
        unsafe {
            self.set_handler_addr(x86_64::VirtAddr::from_ptr(
                save_all_with_err_code_and_ret_isr::<H> as *const (),
            ))
        }
    }
}

impl Isr for Entry<PageFaultHandlerFunc> {
    type Kind = Exception;
    type Ret = ();

    fn register<H>(&mut self, _handler: H) -> &mut EntryOptions
    where
        H: IsrHandler<Self::Kind, Self::Ret>,
    {
        // SAFETY: Handler address is valid for this type of ISR. We guarantee
        // that the ISR will match the expected interrupt stack
        unsafe {
            self.set_handler_addr(x86_64::VirtAddr::from_ptr(
                save_all_with_err_code_and_ret_isr::<H> as *const (),
            ))
        }
    }
}

impl Isr for Entry<DivergingHandlerFunc> {
    type Kind = Interrupt;
    type Ret = Never;

    fn register<H>(&mut self, _handler: H) -> &mut EntryOptions
    where
        H: IsrHandler<Self::Kind, Self::Ret>,
    {
        // SAFETY: Handler address is valid for this type of ISR. We guarantee
        // that the ISR will match the expected interrupt stack
        unsafe {
            self.set_handler_addr(x86_64::VirtAddr::from_ptr(
                save_some_and_diverge_isr::<H> as *const (),
            ))
        }
    }
}

impl Isr for Entry<DivergingHandlerFuncWithErrCode> {
    type Kind = Exception;
    type Ret = Never;

    fn register<H>(&mut self, _handler: H) -> &mut EntryOptions
    where
        H: IsrHandler<Self::Kind, Self::Ret>,
    {
        // SAFETY: Handler address is valid for this type of ISR. We guarantee
        // that the ISR will match the expected interrupt stack
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
    extern "sysv64" fn call(ctx: ExceptionCtx<Kind>) -> Ret {
        const {
            assert!(
                core::mem::size_of::<Self>() == 0,
                "Type is not a zero-sized closure"
            )
        }
        // SAFETY: Since the function is a ZST, a dangling (but well-aligned)
        // pointer to Self is always going to be valid.
        let this: &Self = unsafe { &*core::ptr::dangling() };
        this(ctx)
    }
}

pub struct PanicHandler<const ID: usize>;
impl<const ID: usize, Ret> IsrHandler<Interrupt, Ret> for PanicHandler<ID> {
    extern "sysv64" fn call(ctx: ExceptionCtx<Interrupt>) -> Ret {
        let kind = ExceptionKind::try_from(ID).unwrap();
        panic!("Unhandled interrupt ({kind}) {ctx:#?}")
    }
}

impl<const ID: usize, Ret> IsrHandler<Exception, Ret> for PanicHandler<ID> {
    extern "sysv64" fn call(ctx: ExceptionCtx<Exception>) -> Ret {
        let code = ctx.error_code();
        let kind = ExceptionKind::try_from(ID).unwrap();
        panic!("Unhandled interrupt ({kind}) - Error code: {code:#X}\n{ctx:#?}")
    }
}

// SAFETY: Not actually extern "C". This is more of a x86_interrupt abi
#[unsafe(naked)]
extern "C" fn save_all_and_ret_isr<F: IsrHandler<Interrupt, ()>>() {
    #[allow(unused_unsafe)]
    // SAFETY: Sticking with ISR calling convention. Preserved registers are pushed on
    // call to sysv64 ABI.
    unsafe {
        naked_asm!("swapgs", push_scratch!(), push_preserved!(), "lea rdi, [rsp + 8*15]", "call {inner}", pop_preserved!(), pop_scratch!(), "swapgs", "iretq", inner = sym F::call);
    }
}

// SAFETY: Not actually extern "C". This is more of a x86_interrupt abi
#[unsafe(naked)]
extern "C" fn save_all_with_err_code_and_ret_isr<F: IsrHandler<Exception, ()>>() {
    #[allow(unused_unsafe)]
    // SAFETY: Sticking with ISR calling convention. Preserved registers are pushed on
    // call to sysv64 ABI.
    unsafe {
        naked_asm!("swapgs", push_scratch!(), push_preserved!(), "sub rsp, 8", "lea rdi, [rsp + 8*17]", "call {inner}", "add rsp, 8", pop_preserved!(), pop_scratch!(), "add rsp, 8", "swapgs", "iretq", inner = sym F::call);
    }
}

// SAFETY: Not actually extern "C". This is more of a x86_interrupt abi
#[unsafe(naked)]
extern "C" fn save_some_and_diverge_isr<F: IsrHandler<Interrupt, Never>>() {
    #[allow(unused_unsafe)]
    // SAFETY: Sticking with ISR calling convention. Preserved registers are pushed on
    // call to sysv64 ABI.
    unsafe {
        naked_asm!("swapgs", "sub rsp, 8", "lea rdi, [rsp + 8]", "call {inner}", "ud2", inner = sym F::call);
    }
}

// SAFETY: Not actually extern "C". This is more of a x86_interrupt abi
#[unsafe(naked)]
extern "C" fn save_some_with_err_code_and_diverge_isr<F: IsrHandler<Exception, Never>>() {
    #[allow(unused_unsafe)]
    // SAFETY: Sticking with ISR calling convention. Preserved registers are pushed on
    // call to sysv64 ABI.
    unsafe {
        naked_asm!("swapgs", "lea rdi, [rsp]", "call {inner}", "ud2", inner = sym F::call);
    }
}
