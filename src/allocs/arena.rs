//! This module contains an implementation of a bump-style Arena Allocator.
//! This Arena uses [crate::allocs::page::PageAlloc] to allocate blocks of memory to then allocate
//! into, however it is still generic over the [alloc::alloc::Allocator] trait, so you can use your
//! won allocation strat if you wish

use core::{alloc::Layout, cell::UnsafeCell, ptr::NonNull};

use alloc::alloc::Allocator;

use crate::allocs::page::PageAlloc;

#[repr(C)]
#[derive(Debug)]
struct ArenaInner {
    used: u64,
    mem: [u8],
}

#[derive(Debug)]
#[repr(C)]
struct ArenaRaw<A: Allocator = PageAlloc> {
    inner: NonNull<UnsafeCell<ArenaInner>>,
    alloc: A,
}

impl<A> Drop for ArenaRaw<A>
where
    A: Allocator,
{
    fn drop(&mut self) {
        unsafe {
            let ptr = NonNull::new_unchecked(self.inner.as_ptr() as *mut [u8]);

            let layout =
                Layout::array::<u8>(ptr.len()).expect("Layout should be valid size and align!");

            self.alloc.deallocate(ptr.cast::<u8>(), layout);
        };
    }
}

impl<A> ArenaRaw<A>
where
    A: Allocator,
{
    pub fn new_in(size: usize, alloc: A) -> anyhow::Result<Self> {
        let inner = alloc
            .allocate_zeroed(Layout::array::<u8>(size).expect("Layout size should be valid!"))?;

        let inner =
            unsafe { NonNull::new_unchecked(inner.as_ptr() as *mut UnsafeCell<ArenaInner>) };
        Ok(Self { inner, alloc })
    }

    pub const fn get(&self) -> &UnsafeCell<ArenaInner> {
        unsafe { self.inner.as_ref() }
    }

    pub const fn inner(&self) -> &ArenaInner {
        unsafe { &*self.get().get() }
    }

    pub const fn used_bytes(&self) -> usize {
        self.inner().used as usize
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct Arena<A: Allocator = PageAlloc> {
    inner: ArenaRaw<A>,
}

impl<A> Arena<A>
where
    A: Allocator,
{
    pub fn new_in(size: usize, alloc: A) -> Self {
        let inner = ArenaRaw::new_in(size, alloc)
            .expect("Arena's parent allocator should create it without error!");
        Self { inner }
    }
}

impl Arena<PageAlloc> {
    #[inline(always)]
    pub fn new(size: usize) -> Self {
        Arena::new_in(size, PageAlloc)
    }
}

// unsafe impl<A> Allocator for Arena<A> {
//     fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, alloc::alloc::AllocError> {}
//
//     unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
//         todo!()
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    use crate::megabytes;

    #[test]
    fn arena_works() -> anyhow::Result<()> {
        let a = Arena::new(megabytes(12));
        Ok(())
    }
}
