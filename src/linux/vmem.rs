use core::{
    alloc::Layout,
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use alloc::alloc::Allocator;

use crate::{
    allocs::page::PageAlloc,
    linux::valloc::{VremapMode, vremap},
};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VmemRaw(NonNull<UnsafeCell<[u8]>>);

impl VmemRaw {
    pub const fn allocator() -> PageAlloc {
        PageAlloc
    }

    pub const fn layout_with_size(size: usize) -> Layout {
        let Ok(l) = Layout::array::<u8>(size) else {
            panic!("Layout should have valid size and alignment!");
        };
        l
    }

    pub fn new(size_bytes: usize) -> anyhow::Result<Self> {
        let mut inner = PageAlloc.allocate_zeroed(Self::layout_with_size(size_bytes))?;
        let inner = unsafe { UnsafeCell::from_mut(inner.as_mut()) }; //unsafe { NonNull::new_unchecked(inner.as_ptr() as *mut UnsafeCell<[u8]>) };=
        let inner = NonNull::from_ref(inner);
        Ok(Self(inner))
    }

    pub const fn len(&self) -> usize {
        unsafe { self.0.as_ref().get().len() }
    }

    pub const fn layout(&self) -> Layout {
        Self::layout_with_size(self.len())
    }

    pub const fn inner(&self) -> &UnsafeCell<[u8]> {
        unsafe { self.0.as_ref() }
    }

    pub const fn as_slice_ptr(&self) -> NonNull<[u8]> {
        unsafe { NonNull::new_unchecked(self.inner().get()) }
    }

    pub const fn as_slice(&self) -> &[u8] {
        unsafe { self.as_slice_ptr().as_ref() }
    }

    pub const fn as_mut_slice(&self) -> &[u8] {
        unsafe { self.as_slice_ptr().as_mut() }
    }

    /// @brief uses the [alloc::alloc::Allocator] that [PageAlloc] impls to call
    /// [alloc::alloc::Allocator::grow], or [alloc::alloc::Allocator::shrink],dpending on provided
    /// new_size
    /// @details this method will pass [VremapMode::ResizeInPlace] if new_size is <= current size,
    /// otherwise [VremapMode::AllowMove] is provided to [vremap]
    /// for a version that is simpler, and allows caller to specify [VremapMode],@see
    /// [VmemRaw::remap]
    #[must_use]
    pub fn resize(&mut self, new_size: usize) -> RemapResult {
        let inner = unsafe { NonNull::new_unchecked(self.0.as_ptr() as *mut [u8]) };

        let old_layout = self.layout();
        let new_layout = Self::layout_with_size(new_size);

        if new_layout.size() == old_layout.size() {
            RemapResult::NoChange
        } else if new_layout.size() < old_layout.size() {
            unsafe {
                let Ok(mut inner) = PageAlloc.shrink(inner.cast::<u8>(), old_layout, new_layout)
                else {
                    return RemapResult::Failure;
                };
                self.0 = NonNull::from_ref(UnsafeCell::from_mut(inner.as_mut()));

                RemapResult::Success
            }
        } else {
            unsafe {
                let Ok(mut inner) = PageAlloc.grow(inner.cast::<u8>(), old_layout, new_layout)
                else {
                    return RemapResult::Failure;
                };
                self.0 = NonNull::from_ref(UnsafeCell::from_mut(inner.as_mut()));
            }
            RemapResult::Success
        }
    }

    #[inline]
    pub fn remap(&mut self, new_size: usize, mode: VremapMode) -> RemapResult {
        let ptr = unsafe { vremap(self.as_slice_ptr(), new_size, mode) };
        match (mode, ptr) {
            (VremapMode::ResizeInPlace, None) => RemapResult::MustMove,
            (VremapMode::AllowMove, None) => RemapResult::Failure,

            (VremapMode::AllowMove, Some(ptr)) | (VremapMode::ResizeInPlace, Some(ptr)) => {
                self.0 = unsafe { cast_unsized!(ptr, UnsafeCell<[u8]>) };
                RemapResult::Success
            }
        }
    }

    pub fn try_resize(&mut self, new_size: usize) -> Option<()> {
        self.resize(new_size).ok()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// # SAFETY
    /// Caller must ensure no outstanding references or potinters exist to this virutal memory
    /// poitned to by [VmemRaw] after this funciton returns
    pub unsafe fn free(self) {
        let size = self.len();
        unsafe { PageAlloc.deallocate(self.0.cast::<u8>(), Self::layout_with_size(size)) };
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RemapResult {
    Success,
    /// @brief returned if [VremapMode::ResizeInPlace] was provided and inner [libc::mremap] failed,
    /// thus requiring another call to inner [libc::mremap], providing [VremapMode::AllowMove] in
    /// order to resize
    MustMove,
    /// @brief only returned when new_size == current size
    NoChange,
    #[default]
    Failure,
}

impl RemapResult {
    pub const fn ok(&self) -> Option<()> {
        match self {
            Self::NoChange | Self::Success => Some(()),
            Self::MustMove | Self::Failure => None,
        }
    }
}

/// @brief an absolute byte index to some address in [Vmem]
/// @details you can get this from [Vmem::ptr_at_slot] to convert a reference to
/// [VmemSlot], which you can then use later to persist data through [vremap], where
/// [VremapMode::AllowMove] is provided to it
#[repr(transparent)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VmemSlot(u64);
#[repr(transparent)]
#[derive(Debug)]
pub struct Vmem(VmemRaw);

impl Vmem {
    #[inline(always)]
    pub fn new(size_bytes: usize) -> Self {
        let inner = VmemRaw::new(size_bytes).expect("Vmem should be allocated with no errors!");
        Self(inner)
    }

    pub const fn allocator() -> PageAlloc {
        PageAlloc
    }

    pub fn inner(&self) -> &VmemRaw {
        &self.0
    }

    /// @breif uses [alloc::alloc::Allocator] methods to [vremap]
    /// @details the differnce between this method and [Vmem::remap] is that [Vmem::remap]
    /// doesnt go through [alloc::alloc::Allocator] method implementations
    #[inline(always)]
    pub fn resize(&mut self, new_size: usize) -> RemapResult {
        self.0.resize(new_size)
    }

    #[inline(always)]
    pub fn remap(&mut self, new_size: usize, mode: VremapMode) -> RemapResult {
        self.0.remap(new_size, mode)
    }

    /// @brief points the start of virutal memory block pointed to by [Vmem]
    /// @details for a pointer to the end (+1 byte past it), @see [Vmem::end_ptr]
    #[must_use]
    pub const fn begin_ptr(&self) -> NonNull<u8> {
        self.0.0.cast::<u8>()
    }

    /// @brief points to +1 past the end of virutal memory block pointed to by [Vmem]
    /// @details for a pointer to the beginning, @see [Vmem::begin_ptr]
    #[must_use]
    pub const fn end_ptr(&self) -> NonNull<u8> {
        unsafe { self.begin_ptr().add(self.len()) }
    }

    /// @breif returns [Vmem::begin_ptr] and [Vmem::end_ptr]] together as a tuple, in that order
    #[must_use]
    pub const fn iter_tup(&self) -> (NonNull<u8>, NonNull<u8>) {
        (self.begin_ptr(), self.end_ptr())
    }

    #[inline(always)]
    #[must_use]
    pub fn contains<T>(&self, ptr: &T) -> bool {
        self.contains_ptr(NonNull::from_ref(ptr).cast::<u8>())
    }

    #[inline]
    #[must_use]
    pub fn contains_ptr(&self, ptr: NonNull<u8>) -> bool {
        ptr >= self.begin_ptr() && ptr < self.end_ptr()
    }

    #[inline]
    #[must_use]
    pub fn at_slot<T>(&self, ptr: &T) -> Option<VmemSlot> {
        self.ptr_at_slot(NonNull::from_ref(ptr).cast::<u8>())
    }

    #[must_use]
    pub fn ptr_at_slot(&self, ptr: NonNull<u8>) -> Option<VmemSlot> {
        if self.contains_ptr(ptr) {
            Some(unsafe { self.at_slot_unchecked(ptr) })
        } else {
            None
        }
    }

    /// # SAFETY
    /// This function makes no safety checks to ensure given poitner actually belongs to this region
    /// of virutal memory pointed to by [Vmem], as such the caller must ensure that the pointer
    /// provided indeed does belong to this [Vmem], not doing so will provide invalid data, and
    /// therefore likely to cause UB!
    #[inline]
    #[must_use]
    pub unsafe fn at_slot_unchecked(&self, ptr: NonNull<u8>) -> VmemSlot {
        let begin = self.begin_ptr();
        let slot = ptr.addr().get() - begin.addr().get();
        VmemSlot(slot as u64)
    }

    pub const fn len(&self) -> usize {
        self.0.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub const fn layout_with_size(size: usize) -> Layout {
        VmemRaw::layout_with_size(size)
    }

    pub const fn layout(&self) -> Layout {
        Self::layout_with_size(self.len())
    }

    pub const fn get(&self) -> &UnsafeCell<[u8]> {
        unsafe { self.0.0.as_ref() }
    }

    pub const fn as_slice(&self) -> &[u8] {
        unsafe { self.get().get().as_ref().unwrap() }
    }

    pub const fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { self.get().get().as_mut().unwrap() }
    }
}

impl DerefMut for Vmem {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl Deref for Vmem {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl Drop for Vmem {
    #[inline]
    fn drop(&mut self) {
        unsafe { VmemRaw::free(self.0) };
    }
}
