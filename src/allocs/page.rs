use core::{alloc::Layout, ptr::NonNull};

use alloc::alloc::Allocator;
use anyhow::Context;

use crate::{
    layout_bytes,
    linux::valloc::{VremapMode, valloc, vfree, vremap},
    valloc_unsized,
};

// @brief Deals in chunks of [crate::os::page_size]
/// @details Requested allocations that are smaller than [crate::os::page_size] are rounded up to
/// said page size, so be mindful of using this allocator to allocate blocks of memory that are
/// smaller than page size
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageAlloc;

impl PageAlloc {
    pub const fn new() -> Self {
        Self
    }

    pub fn valloc(size: usize) -> anyhow::Result<NonNull<[u8]>> {
        valloc_unsized!(size, [u8])
    }

    pub fn resize(
        &self,
        ptr: NonNull<[u8]>,
        new_size: usize,
        mode: VremapMode,
    ) -> Option<NonNull<[u8]>> {
        let old = layout_bytes(ptr.len());
        let layout = layout_bytes(new_size);
        self.do_resize(ptr.cast::<u8>(), old, layout).ok()
    }

    fn do_resize(
        &self,
        ptr: NonNull<u8>,
        old: Layout,
        new: Layout,
    ) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {
        let mode = if new.size() <= old.size() {
            VremapMode::ResizeInPlace
        } else {
            VremapMode::AllowMove
        };

        let ptr = NonNull::slice_from_raw_parts(ptr, old.size());
        let ptr = unsafe { vremap(ptr, new.size(), mode).ok_or(alloc::alloc::AllocError)? };
        Ok(ptr)
    }
}

unsafe impl Allocator for PageAlloc {
    fn allocate(
        &self,
        layout: core::alloc::Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, alloc::alloc::AllocError> {
        self.allocate_zeroed(layout)
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        let ptr = NonNull::slice_from_raw_parts(ptr, layout.size());
        unsafe { vfree(ptr).expect("munmap should execute without error") };
    }

    fn allocate_zeroed(
        &self,
        layout: core::alloc::Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, alloc::alloc::AllocError> {
        let ptr = unsafe {
            valloc(layout.size(), crate::linux::valloc::VmapMode::PrivateAnon)
                .map_err(|_| alloc::alloc::AllocError)?
        };
        Ok(ptr)
    }

    unsafe fn grow(
        &self,
        ptr: core::ptr::NonNull<u8>,
        old_layout: core::alloc::Layout,
        new_layout: core::alloc::Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, alloc::alloc::AllocError> {
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );
        unsafe { self.grow_zeroed(ptr, old_layout, new_layout) }
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: core::ptr::NonNull<u8>,
        old_layout: core::alloc::Layout,
        new_layout: core::alloc::Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, alloc::alloc::AllocError> {
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );
        self.do_resize(ptr, old_layout, new_layout)
    }

    unsafe fn shrink(
        &self,
        ptr: core::ptr::NonNull<u8>,
        old_layout: core::alloc::Layout,
        new_layout: core::alloc::Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, alloc::alloc::AllocError> {
        debug_assert!(
            new_layout.size() <= old_layout.size(),
            "`new_layout.size()` must be smaller than or equal to `old_layout.size()`"
        );

        self.do_resize(ptr, old_layout, new_layout)
    }
}
