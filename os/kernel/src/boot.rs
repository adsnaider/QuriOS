//! Boot process initialization

pub(crate) mod bump_alloc;

use core::mem::MaybeUninit;
use core::ops::Range;

use bump_alloc::{BumpFrameAllocator, OutOfMemory};
use derive_more::{Display, Error, From};
use loader::{ElfError, Loader, LoaderError, MemFlags, Program, SegmentLoadError};
use qapi::init::{BootArgs, EntryFn, RetypeEntry};
use qapi::types::CSlice;
use zerocopy::IntoBytes as _;

use crate::arch::exec::ExecState;
use crate::arch::mem::{
    Addrspace, Frame, FrameAllocError, MapPageError, Page, PageFlags, VirtAddr,
    UNTYPED_MEMORY_OFFSET,
};
use crate::arch::System;
use crate::pmo::PhysAddrExt as _;
use crate::retyping::{FrameExt, RetypeTable};

#[derive(Debug)]
pub struct Process<S: System> {
    pub exec: S::ExecState,
    pub addrspace: S::Addrspace,
}

#[derive(Debug)]
pub struct InitLoader<'a, A> {
    address_space: &'a A,
    fallocator: &'a mut BumpFrameAllocator,
}

#[derive(Debug, Error, Display, From)]
pub enum LoadPageError {
    AllocError(FrameAllocError),
    MapError(MapPageError),
    OutOfMemory(OutOfMemory),
}

#[derive(Debug, Error, Display, From)]
#[allow(clippy::enum_variant_names)]
pub enum LoadError {
    ElfError(ElfError),
    ElfSegmentLoadError(SegmentLoadError<LoadPageError>),
    ElfSourceLoadError(LoaderError<LoadPageError>),
}

impl<A: Addrspace> InitLoader<'_, A> {
    fn request_page(
        &mut self,
        page: Page,
        rwx: MemFlags,
    ) -> Result<&mut [MaybeUninit<u8>], LoadPageError> {
        let pflags = rwx.into();
        let frame = self.fallocator.alloc_user_frame()?;
        log::trace!("Mapping {page:?} to {frame:?} with {pflags:?}");
        assert!(
            page.base().is_lower_half(),
            "All kernel memory should remain the same"
        );
        // SAFETY: We properly allocated an unused frame and the page will only be used on the loaded process.
        let frame = unsafe {
            match self.address_space.map_page(
                page,
                frame.raw(),
                pflags,
                // Parent flags are the least restrictive since they will be reused for many pages.
                PageFlags::all(),
                self.fallocator,
            ) {
                Ok(f) => {
                    f.flush();
                    frame.into_raw()
                }
                Err(MapPageError::AlreadyMapped(f)) => f,
                Err(e) => return Err(e.into()),
            }
        };
        // SAFETY: The frame was newly allocated for userspace
        Ok(unsafe {
            core::slice::from_raw_parts_mut(frame.base().to_virtual().as_mut_ptr(), Page::SIZE)
        })
    }

    /// # Safety
    ///
    /// It must be okay for the frame to be mapped to userspace with the provided flags
    unsafe fn map_page(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PageFlags,
    ) -> Result<(), LoadPageError> {
        // SAFETY: Precondition
        unsafe {
            self.address_space
                .map_page(
                    page,
                    frame,
                    flags,
                    // Parent flags are the least restrictive since they will be reused for many pages.
                    PageFlags::all(),
                    self.fallocator,
                )?
                .ignore();
        }
        Ok(())
    }
}

impl<'a, A: Addrspace> Loader for InitLoader<'a, A> {
    type Error = LoadPageError;
    fn load_with<F>(
        &mut self,
        at: Range<usize>,
        source: F,
        rwx: MemFlags,
    ) -> Result<(), Self::Error>
    where
        F: Fn(usize) -> MaybeUninit<u8>,
    {
        let mut offset = 0;
        let start_page = at.start / Page::SIZE;
        let end_page = at.end.div_ceil(Page::SIZE);
        for page in start_page..end_page {
            let page = Page::from_index(page).unwrap();
            let dest = self.request_page(page, rwx)?;

            let dest_range = ((at.start + offset) % Page::SIZE)..Page::SIZE;
            let source_range = offset..at.len();

            for (source_off, dest_off) in source_range.zip(dest_range) {
                dest[dest_off] = source(source_off);
                offset += 1;
            }
        }
        Ok(())
    }

    unsafe fn unload(&mut self, _vrange: Range<usize>) {
        unimplemented!()
    }
}
impl<S: System> Process<S> {
    pub fn load(
        sys: &S,
        program: &[u8],
        stack_pages: usize,
        initrd: &[u8],
        fallocator: &mut BumpFrameAllocator,
    ) -> Result<Self, LoadError> {
        let untyped_memory_offset = UNTYPED_MEMORY_OFFSET;
        let untyped_memory_length = Frame::memory_limit();
        assert!(untyped_memory_offset % Page::SIZE == 0);
        assert!(untyped_memory_length % Page::SIZE == 0);
        assert!(untyped_memory_offset + untyped_memory_length < 0xFFFF_8000_0000_0000);
        let program = Program::new(program)?;

        // SAFETY: The addrespace is accounted for at retype init with index 1.
        let addrspace = sys.addrspace();

        let mut loader = InitLoader {
            address_space: &addrspace,
            fallocator,
        };
        log::info!("Loading process headers");
        let process = program.load(&mut loader)?;
        log::debug!("Entry: {:#X}", process.entry());

        let stack_top = untyped_memory_offset;
        let stack_bottom = untyped_memory_offset
            .checked_sub(stack_pages * Page::SIZE)
            .unwrap();
        log::info!("Setting up stack pages at {:#X?}", stack_bottom..stack_top);
        loader
            .load_zeroed(stack_bottom..stack_top, MemFlags::READ | MemFlags::WRITE)
            .unwrap();

        let untyped_pages = untyped_memory_length / Page::SIZE;
        let untyped_start = Page::from_start_address(VirtAddr::new(untyped_memory_offset)).index();
        log::info!("Setting up {untyped_pages} untyped pages at {untyped_memory_offset:#X?}",);
        for (frame, page) in (untyped_start..(untyped_start + untyped_pages)).enumerate() {
            let page = Page::from_index(page).unwrap();
            let frame = Frame::from_index(frame as u64).unwrap();
            // SAFETY: Userspace does not have access to the underlying frames.
            unsafe { loader.map_page(page, frame, PageFlags::PRESENT).unwrap() };
        }

        let retype_table_metadata = RetypeTable::memory_map_meta();
        let memory_map_start = process.top_of_text().div_ceil(Page::SIZE) * Page::SIZE;
        log::info!("Loading memory map at {:?}", memory_map_start as *const u8);
        let mut memory_map_count = 0;
        for frame in retype_table_metadata.frames {
            let page = Page::from_start_address(VirtAddr::new(
                memory_map_start + memory_map_count * Page::SIZE,
            ));
            log::trace!("Loading memory map at {page:?}");
            // SAFETY: Userspace will only  be given read access to the memory map
            unsafe {
                loader
                    .map_page(
                        page,
                        frame,
                        PageFlags::PRESENT | PageFlags::READABLE | PageFlags::USER_ACCESSIBLE,
                    )
                    .unwrap()
            };
            memory_map_count += 1;
        }
        let memory_map_end = memory_map_start + memory_map_count * Page::SIZE;
        assert!(memory_map_end % Page::SIZE == 0);
        // And also pass through the initrd image
        let initrd_start = memory_map_end;
        let initrd_end = initrd_start + initrd.len();
        log::info!("Loading initrd at {:?}", initrd_start as *const u8);
        loader.load_source(initrd_start..initrd_end, initrd, MemFlags::READ)?;

        let bootargs = BootArgs::new(
            // SAFETY: We loaded the memory map entries to this memory location
            unsafe {
                CSlice::from_raw_parts(
                    memory_map_start as *const RetypeEntry,
                    retype_table_metadata.map.1,
                )
            },
            // SAFETY: We loaded the initrd to this memory location
            unsafe { CSlice::from_raw_parts(initrd_start as *const u8, initrd.len()) },
            initrd_end,
        );

        let bootargs_start = initrd_end.div_ceil(size_of::<BootArgs>()) * size_of::<BootArgs>();
        let bootargs_end = bootargs_start + size_of::<BootArgs>();
        log::info!(
            "Loading boot arguments at {:?}",
            bootargs_start as *const u8
        );
        loader.load_source(
            bootargs_start..bootargs_end,
            bootargs.as_bytes(),
            MemFlags::READ,
        )?;

        // SAFETY: If the address is not properly defined in the user process it might trigger
        // UB but that is in userspace, not in the kernel since that is an extern
        // function in a different memory space
        let boot_fn: EntryFn = unsafe { core::mem::transmute(process.entry()) };

        log::info!("Initialized user process");
        Ok(Self {
            addrspace,
            exec: S::ExecState::for_init_comp(
                boot_fn,
                stack_top as *const (),
                bootargs_start as *const BootArgs,
            ),
        })
    }
}
