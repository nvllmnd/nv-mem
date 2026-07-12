use core::{
    alloc::Allocator,
    ops::{Deref, DerefMut},
};

use alloc::{alloc::Global, vec::Vec};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct String<A: Allocator> {
    mem: Vec<u8, A>,
}

/// @brief Drop-in Replacement for core::String that is generic over the [Allocator] Trait
impl<A> Deref for String<A>
where
    A: Allocator,
{
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        core::str::from_utf8(self.mem.as_slice()).expect("String should be valid UTF-8!")
    }
}

impl<A> DerefMut for String<A>
where
    A: Allocator,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        core::str::from_utf8_mut(self.mem.as_mut_slice()).expect("String should be valid UTF-8!")
    }
}

impl<A> String<A>
where
    A: Allocator,
{
    pub const fn new_in(alloc: A) -> Self {
        Self {
            mem: Vec::new_in(alloc),
        }
    }

    pub fn from_string(s: alloc::string::String, alloc: A) -> Self {
        let bytes = s.as_bytes();
        let mut mem = Vec::with_capacity_in(bytes.len(), alloc);

        unsafe { mem.set_len(bytes.len()) };
        mem.clone_from_slice(bytes);

        Self { mem }
    }

    pub fn with_capacity_in(cap: usize, alloc: A) -> Self {
        Self {
            mem: Vec::with_capacity_in(cap, alloc),
        }
    }

    #[inline]
    pub fn allocator(&self) -> A
    where
        A: Clone,
    {
        A::clone(self.mem.allocator())
    }

    #[inline]
    pub fn allocator_ref(&self) -> &A {
        self.mem.allocator()
    }

    pub fn to_string(self) -> alloc::string::String {
        let (ptr, len, cap, _alloc) = self.mem.into_raw_parts_with_alloc();
        unsafe { alloc::string::String::from_raw_parts(ptr, len, cap) }
    }

    #[inline]
    pub unsafe fn set_len(&mut self, len: usize) {
        unsafe { self.mem.set_len(len) }
    }
}

impl<A> AsRef<str> for String<A>
where
    A: Allocator,
{
    fn as_ref(&self) -> &str {
        self.deref()
    }
}

impl<A> AsMut<str> for String<A>
where
    A: Allocator,
{
    fn as_mut(&mut self) -> &mut str {
        self.deref_mut()
    }
}

impl String<Global> {
    pub const fn new() -> Self {
        Self { mem: Vec::new() }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            mem: Vec::with_capacity(cap),
        }
    }

    pub fn to_string_in<A>(&self, alloc: A) -> String<A>
    where
        A: Allocator,
    {
        let mut s = String::<A>::new_in(alloc);
        unsafe { s.set_len(self.len()) };
        unsafe { s.as_bytes_mut().copy_from_slice(self.as_bytes()) };
        s
    }
}

impl Default for String<Global> {
    fn default() -> Self {
        Self::new()
    }
}
