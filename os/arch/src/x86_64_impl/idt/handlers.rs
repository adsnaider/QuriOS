mod asm_utils;
use asm_utils::{pop_scratch, push_scratch};

use core::{arch::naked_asm, marker::PhantomData};

use x86_64::structures::idt::{Entry, EntryOptions, HandlerFunc};

pub struct InterruptHandler<I> {
    fun: PhantomData<I>,
}

pub trait Isr: Sized {
    fn call();
    fn register<F>(entry: &mut Entry<F>, _isr: Self) -> &mut EntryOptions {
        unsafe { InterruptHandler::<Self>::set_handler(entry) }
    }
}

impl<I: Isr> InterruptHandler<I> {
    pub unsafe fn set_handler<F>(entry: &mut Entry<F>) -> &mut EntryOptions {
        // SAFETY: The provided shim is correct and valid.
        unsafe { entry.set_handler_addr(x86_64::VirtAddr::from_ptr(Self::isr as *const ())) }
    }

    #[unsafe(naked)]
    extern "C" fn isr() {
        #[allow(unused_unsafe)]
        // SAFETY: Sticking with ISR calling convention. Preserved registers are pushed on
        // call to sysv64 ABI.
        unsafe {
            naked_asm!(push_scratch!(), "call {inner}", pop_scratch!(), "iretq", inner = sym Self::inner);
        }
    }

    extern "sysv64" fn inner() {
        I::call();
    }
}

pub struct PanicHandler<const ID: usize>;
impl<const ID: usize> Isr for PanicHandler<ID> {
    fn call() {
        unimplemented!("The requested interrupt (ID: {ID}) is not yet implemented");
    }
}

impl<F: Fn()> Isr for F
where
    F: Zst,
{
    fn call() {
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
        #[allow(clippy::let_unit_value)]
        let _ = Self::ZST;
        debug_assert!(core::mem::size_of::<Self>() == 0);
    }
}

impl<T> Zst for T {}
