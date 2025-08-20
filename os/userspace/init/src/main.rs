#![no_std]
#![no_main]

use core::arch::naked_asm;
use core::mem::MaybeUninit;

use allocator_api2::boxed::Box;
use entry::entry;
use qapi::caps::slotid::SlotId;
use qapi::caps::sync_ipc::SyncInvokeCap;
use qapi::caps::{CapId, PositiveIsize};
use qapi::init::{BootArgs, BootCaps};
use qapi::syscall::SyscallOp;
use qapi::syscall::ops::sync_ipc::{SYNC_CALL_ARGS, SyncCallFun};
use stack_list::{StackList, StackNode, stack_list_pop, stack_list_push};
use ulib::alloc::allocman::{ALockedMan, Allocman, ReservedHeap};
use ulib::alloc::caps::CapabilityMan;
use ulib::alloc::phys::bitmap_allocator::BitmapAllocator;
use ulib::alloc::virt::Addrspace;
use ulib::sysops::{CTableCapExt, CapIdExt, SyncInvokeCapExt};

#[global_allocator]
static ALLOCATOR: ALockedMan<BitmapAllocator> = ALockedMan::uninit();

static STACK_LIST: StackList<'static> = StackList::new();

#[entry]
fn main(args: &'static BootArgs) -> ! {
    {
        static mut SNODE1: [u128; 4096] = [0; 4096];
        #[allow(static_mut_refs)]
        STACK_LIST.push_front(StackNode::new(unsafe { &mut SNODE1 }).unwrap());
    }
    serial::init();
    log::info!("Landed on userspace init");
    let bootcaps = BootCaps::new();
    bootcaps
        .self_caps
        .make_sync_call(
            SlotId::new(10).unwrap(),
            make_sync_call(sync_invoke),
            bootcaps.self_addrspace,
            bootcaps.self_caps,
        )
        .unwrap();

    let sync_cap = SyncInvokeCap::new(CapId::new(10));
    let result = sync_cap
        .invoke([
            MaybeUninit::new(11),
            MaybeUninit::new(12),
            MaybeUninit::new(13),
            MaybeUninit::new(14),
        ])
        .unwrap();
    log::info!("Sync response: {result}");

    /*
    let cspace = CapabilityMan::new_starting_at(bootcaps.self_caps, BootCaps::next_free());

    for i in 0..BootCaps::next_free().as_usize() {
        log::info!(
            "Introspecting capability ({i}): {:#?}",
            CapId::new(i as u32).introspect()
        )
    }



    let falloc = BitmapAllocator::new(
        // bootcaps.self_addrspace,
        args.memory_map.as_slice(),
        // args.free_space_start,
    );
    let vspace = Addrspace::new(bootcaps.self_addrspace, ReservedHeap::empty());
    ALLOCATOR.set(Allocman::new(falloc, vspace, cspace));

    let foo = Box::new(10);
    assert_eq!(*foo, 10);
    */

    todo!();
}

extern "C" fn make_sync_call<F>(_fun: F) -> SyncCallFun
where
    F: Fn(usize, usize, usize, usize) -> PositiveIsize,
{
    extern "C" fn inner<F>(a: usize, b: usize, c: usize, d: usize) -> PositiveIsize
    where
        F: Fn(usize, usize, usize, usize) -> PositiveIsize,
    {
        const { assert!(core::mem::size_of::<F>() == 0) }
        let f: *const F = core::ptr::dangling();
        unsafe { (*f)(a, b, c, d) }
    }
    #[unsafe(naked)]
    extern "C" fn entry<F>(a: usize, b: usize, c: usize, d: usize) -> PositiveIsize
    where
        F: Fn(usize, usize, usize, usize) -> PositiveIsize,
    {
        #[allow(unused_unsafe)]
        unsafe {
            naked_asm!(
                "mov r12, rdi",
                "mov r13, rsi",
                "mov r14, rdx",
                "mov r15, rcx",
                "lea rdi, [{stack_list}]",
                stack_list_pop!(),
                "test rax, rax",
                "je 3f",
                "mov rsp, rax",
                "mov rdi, r12",
                "mov rsi, r13",
                "mov rdx, r14",
                "mov rcx, r15",
                "call {inner}",
                "mov r12, rax",
                "lea rdi, [{stack_list}]",
                "mov rsi, rsp",
                stack_list_push!(),
                "mov rax, r12",
                "jmp 4f",
                "3:",
                 "mov rax, 1",
                "4:",
                 "mov rsp, 0",
                 "mov rdi, {sync_ret_call}",
                 "mov rsi, rax",
                "int 0x80",
                "ud2",
                inner = sym inner::<F>,
                stack_list = sym STACK_LIST,
                sync_ret_call = const { SyscallOp::SyncRet as usize },
            )
        }
    }
    entry::<F>
}

fn sync_invoke(a: usize, b: usize, c: usize, d: usize) -> PositiveIsize {
    log::info!("Synchronous call: ({a}, {b}, {c}, {d})");
    10isize.try_into().unwrap()
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use serial::sprintln;
    sprintln!("{}", info);
    loop {}
}
