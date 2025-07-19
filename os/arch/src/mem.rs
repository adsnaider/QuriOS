pub mod phys;
pub mod pmo;
pub mod virt;

use core::{arch::naked_asm, marker::PhantomData, sync::atomic::AtomicBool};

use bitflags::bitflags;

use derive_more::{Display, Error, From};
pub use phys::{Frame, PhysAddr};
pub use pmo::Pmo;
pub use virt::{MemorySegment, Page, VirtAddr};

pub const UNTYPED_MEMORY_OFFSET: usize = 0x0000_7000_0000_0000;
pub const HIGHER_HALF: usize = 0xFFFF_8000_0000_0000;

#[derive(Debug, Error, Display)]
pub enum FrameAllocError {
    #[display("No more frames in the system")]
    OutOfMemory,
}

pub trait FrameAllocator {
    fn alloc_kernel_frame(&mut self) -> Result<Frame, FrameAllocError>;
}

#[derive(Debug, Error, Display, From)]
pub enum MapPageError {
    #[from]
    AllocError(FrameAllocError),
    #[display("Tried to map over a huge entry")]
    HugeParentEntry,
    #[display("Tried to map an already mapped entry")]
    AlreadyMapped(#[error(not(source))] Frame),
}

pub trait Addrspace: Sized {
    /// Maps a page to the given frame for the provided addrspace.
    ///
    /// # Safety
    ///
    /// Making modifications to the addrspace may result in UB. This can be avoided
    /// by not modifying any higher-half (kernel-owned) memory.
    unsafe fn map_page<A: FrameAllocator>(
        &self,
        page: Page,
        frame: Frame,
        flags: PageFlags,
        parent_flags: PageFlags,
        alloc: &mut A,
    ) -> Result<Flusher<Self>, MapPageError>;

    fn translate_page(&self, page: Page) -> Option<(Frame, PageFlags)>;

    // TODO: Support huge pages as well

    /// Flushes the given page, forcing the addrspace changes to go into effect
    fn flush(page: Page);

    fn activate(&self);

    fn into_frame(self) -> Frame;

    fn from_frame(pmo: &Pmo, frame: Frame) -> Self;
}

#[must_use]
pub struct Flusher<A: ?Sized>(Page, PhantomData<A>);

impl<A: Addrspace> Flusher<A> {
    pub fn new(page: Page) -> Self {
        Self(page, PhantomData)
    }

    pub fn flush(&self) {
        A::flush(self.0);
    }

    pub fn ignore(self) {}
}

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct PageFlags: u64 {
        const PRESENT = 1;
        const READABLE = 1 << 1;
        const WRITABLE = 1 << 2;
        const EXECUTABLE = 1 << 3;
        const USER_ACCESSIBLE = 1 << 4;
    }
}

impl PageFlags {
    pub fn readable(&self) -> bool {
        self.contains(PageFlags::READABLE)
    }

    pub fn writeable(&self) -> bool {
        self.contains(PageFlags::WRITABLE)
    }

    pub fn executable(&self) -> bool {
        self.contains(PageFlags::EXECUTABLE)
    }

    pub fn present(&self) -> bool {
        self.contains(PageFlags::PRESENT)
    }

    pub fn user_accessible(&self) -> bool {
        self.contains(PageFlags::USER_ACCESSIBLE)
    }
}

impl From<loader::MemFlags> for PageFlags {
    fn from(rwx: loader::MemFlags) -> Self {
        let mut pflags = PageFlags::PRESENT | PageFlags::USER_ACCESSIBLE;
        if rwx.readable() {
            pflags |= PageFlags::WRITABLE;
        }
        if rwx.writeable() {
            pflags |= PageFlags::WRITABLE;
        }
        if rwx.executable() {
            pflags |= PageFlags::EXECUTABLE;
        }
        pflags
    }
}

pub(crate) static mut USER_BUFFER_SAFE_READ: bool = false;

#[unsafe(naked)]
pub unsafe extern "C" fn user_buffer_read(
    kernel_buffer: *mut u8,
    user_buffer: *const u8,
    length: usize,
) -> bool {
    naked_asm!(
        "        mov     rax, qword ptr [rip + {trap_flag}@GOTPCREL] ",
        "        mov     byte ptr [rax], 1 ",
        "        test    rdx, rdx ",
        "        je      6f",
        "        mov     ecx, edx ",
        "        and     ecx, 3 ",
        "        cmp     rdx, 4 ",
        "        jae     2f",
        "        xor     r8d, r8d ",
        "        jmp     4f",
        "2: ",
        "        and     rdx, -4 ",
        "        xor     r9d, r9d ",
        "3: ",
        "        movzx   r8d, byte ptr [rsi + r9] ",
        "        mov     byte ptr [rdi + r9], r8b ",
        "        movzx   r8d, byte ptr [rsi + r9 + 1] ",
        "        mov     byte ptr [rdi + r9 + 1], r8b ",
        "        movzx   r8d, byte ptr [rsi + r9 + 2] ",
        "        mov     byte ptr [rdi + r9 + 2], r8b ",
        "        lea     r8, [r9 + 4] ",
        "        movzx   r10d, byte ptr [rsi + r9 + 3] ",
        "        mov     byte ptr [rdi + r9 + 3], r10b ",
        "        mov     r9, r8 ",
        "        cmp     rdx, r8 ",
        "        jne     3b ",
        "4: ",
        "        test    rcx, rcx ",
        "        je      6f ",
        "        add     rdi, r8 ",
        "        add     rsi, r8 ",
        "        xor     edx, edx ",
        "5: ",
        "        movzx   r8d, byte ptr [rsi + rdx] ",
        "        mov     byte ptr [rdi + rdx], r8b ",
        "        inc     rdx ",
        "        cmp     rcx, rdx ",
        "        jne     5b",
        "6: ",
        "        mov     byte ptr [rax], 0 ",
        "        mov     al, 1 ",
        "        ret ",
        trap_flag = sym USER_BUFFER_SAFE_READ
    );
}

#[unsafe(naked)]
pub(crate) unsafe extern "C" fn user_buffer_read_page_fault_call_gate() {
    naked_asm!(
        "mov rax, qword ptr [rip + {trap_flag}@GOTPCREL]",
        "mov byte ptr [rax], 0 ",
        "mov al, 0",
        "ret",
        trap_flag = sym USER_BUFFER_SAFE_READ
    );
}
