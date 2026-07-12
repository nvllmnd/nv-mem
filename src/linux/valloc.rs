use core::ptr::{NonNull, null_mut};

use anyhow::bail;

use crate::layout_unsized;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VmapMode {
    #[default]
    PrivateAnon,
    /// @brief imples [VmapMode::PrivateAnon] as well
    NoReserve,
}

/// # SAFETY
/// memory returned is private anonymous virtual memory
pub unsafe fn valloc(size_bytes: usize, mode: VmapMode) -> anyhow::Result<NonNull<[u8]>> {
    let size = core::cmp::max(crate::linux::page_size(), size_bytes);
    let flags = match mode {
        VmapMode::PrivateAnon => libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
        VmapMode::NoReserve => libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE,
    };

    let ptr = unsafe {
        let ptr = libc::mmap(
            null_mut(),
            size_bytes,
            libc::PROT_READ | libc::PROT_WRITE,
            flags,
            -1,
            0,
        );
        if ptr == libc::MAP_FAILED {
            bail!("mmap of size: {size} bytes returned MAP_FAILED!");
        }
        NonNull::new(ptr.cast::<u8>()).expect("pointer returned from mmap should not be null!")
    };
    let ptr = NonNull::slice_from_raw_parts(ptr, size);
    Ok(ptr)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VremapMode {
    #[default]
    ResizeInPlace,
    AllowMove,
}

/// # SAFETY
/// [None] indicates failure to resize in place (most likely) or some other error if
/// [VremapMode::AllowMove] was provided
pub unsafe fn vremap(
    ptr: NonNull<[u8]>,
    new_size: usize,
    mode: VremapMode,
) -> Option<NonNull<[u8]>> {
    let flags = match mode {
        VremapMode::ResizeInPlace => 0,
        VremapMode::AllowMove => libc::MREMAP_MAYMOVE,
    };

    let ptr = unsafe {
        let ptr = libc::mremap(
            ptr.cast::<libc::c_void>().as_ptr(),
            ptr.len(),
            new_size,
            flags,
        );
        if ptr == libc::MAP_FAILED {
            return None;
        }
        NonNull::new(ptr.cast::<u8>()).expect("Pointer returned by mremap should not be null!")
    };
    let ptr = NonNull::slice_from_raw_parts(ptr, new_size);
    Some(ptr)
}

/// # SAFETY
/// pointer passed to this function must have been returned by a call to [valloc] or [vremap]
pub unsafe fn vfree(ptr: NonNull<[u8]>) -> anyhow::Result<()> {
    let size = ptr.len();
    unsafe {
        let err = libc::munmap(ptr.as_ptr().cast::<libc::c_void>(), size);
        if err == -1 {
            bail!(
                "Failed to munmap virtual memory of size: {size}. Ensure size is a power of 2 and poitner is page aligned!"
            );
        }
    };
    Ok(())
}

#[macro_export]
macro_rules! alloc_unsized {
    ($alloc: expr, $size: expr, $ty: ty) => {{
        fn do_alloc_unsized_in<A: alloc::alloc::Allocator>(
            size: usize,
            alloc: &A,
        ) -> anyhow::Result<core::ptr::NonNull<$ty>> {
            unsafe {
                let layout = $crate::layout_unsized(ptr.as_ref());
                let ptr = ptr.cast::<u8>();
                let ptr = alloc.allocate(layout)?;
                Ok(core::ptr::NonNull::new_unchecked(ptr.as_ptr() as *mut $ty))
            }
        }
        do_alloc_unsized_in($size, &$alloc)
    }};
}

#[macro_export]
macro_rules! free_unsized {
    ($alloc: expr, $size: expr, $ty: ty) => {{
        fn do_free_unsized_in<A: alloc::alloc::Allocator>(
            ptr: core::ptr::NonNull<$ty>,
            size: usize,
            alloc: &A,
        ) -> anyhow::Result<()> {
            unsafe {
                let layout = $crate::layout_unsized(ptr.as_ref());
                let ptr = ptr.cast::<u8>();
                alloc.deallocate(ptr, layout)?;
                Ok(())
            }
        }
        do_free_unsized_in($size, &$alloc)
    }};
}

#[macro_export]
macro_rules! grow_unsized {
    ($alloc: expr, $size: expr, $ty: ty) => {{
        fn do_grow_unsized_in<A: alloc::alloc::Allocator>(
            ptr: core::ptr::NonNull<$ty>,
            new_size: usize,
            alloc: &A,
        ) -> anyhow::Result<NonNull<$ty>> {
            unsafe {
                let layout = $crate::layout_unsized(ptr.as_ref());
                let new = $crate::layout_bytes(new_size);
                let ptr = ptr.cast::<u8>();
                let ptr = alloc.grow(ptr, layout, new)?;
                Ok(())
            }
        }
        do_free_unsized_in($size, &$alloc)
    }};
}

#[macro_export]
macro_rules! valloc_unsized {
    ($size: expr, $ty: ty) => {{
        // $crate::os::alloc_in_unsized!($ty, Layout::array::<u8>($size).unwrap(), PageAllocator)
        #[inline(always)]
        fn do_alloc_unsized(size: usize) -> anyhow::Result<core::ptr::NonNull<$ty>> {
            unsafe {
                let ptr = $crate::linux::valloc::valloc(
                    size,
                    $crate::linux::valloc::VmapMode::default(),
                )?;
                Ok(core::ptr::NonNull::new_unchecked(ptr.as_ptr() as *mut $ty))
            }
        }

        do_alloc_unsized($size)
    }};
}
#[macro_export]
macro_rules! vremap_unsized {
    ($ptr: expr, $new_size: expr, $mode: expr, $ty: ty) => {{
        // $crate::os::alloc_in_unsized!($ty, Layout::array::<u8>($size).unwrap(), PageAllocator)
        #[inline(always)]
        fn do_remap_unsized(
            ptr: core::ptr::NonNull<$ty>,
            new_size: usize,
            mode: $crate::linux::valloc::VremapMode,
        ) -> Option<NonNull<$ty>> {
            unsafe {
                let layout = $crate::layout_unsized(ptr.as_ref());
                let ptr = core::ptr::NonNull::slice_from_raw_parts(ptr.cast::<u8>(), layout.size());
                let ptr = $crate::linux::valloc::vremap(ptr, new_size, mode)?;
                Some(NonNull::new_unchecked(ptr.as_ptr() as *mut $ty))
            }
        }

        do_remap_unsized($ptr, $new_size, $mode)
    }};
}

#[inline]
pub fn vfree_unsized<T: ?Sized>(ptr: NonNull<T>) -> anyhow::Result<()> {
    unsafe {
        let layout = layout_unsized(ptr.as_ref());
        let ptr = NonNull::slice_from_raw_parts(ptr.cast::<u8>(), layout.size());
        vfree(ptr)
    }
}
