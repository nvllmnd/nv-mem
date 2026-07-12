#![no_std]
#![feature(allocator_api)]

use core::alloc::Layout;

extern crate alloc;

pub const KB1: usize = 1024;

pub const fn kilobytes(n: usize) -> usize {
    n * KB1
}

pub const fn megabytes(n: usize) -> usize {
    kilobytes(n) * KB1
}

pub const fn gigabytes(n: usize) -> usize {
    megabytes(n) * KB1
}

pub const fn terabytes(n: usize) -> usize {
    gigabytes(n) * KB1
}

pub const fn layout_bytes(size: usize) -> Layout {
    let Ok(l) = Layout::array::<u8>(size) else {
        panic!("Layout of bytes (u8) should have size less than isize::MAX!");
    };
    l
}

pub const fn layout_unsized<T: ?Sized>(val: &T) -> Layout {
    let size = core::mem::size_of_val(val);
    let align = core::mem::align_of_val(val);
    let Ok(l) = Layout::from_size_align(size, align) else {
        panic!("Layout of unsized type must have valid size and alignment!");
    };
    l
}

#[macro_export]
macro_rules! cast_unsized {
    ($ptr: expr, $ty: ty) => {{ NonNull::new_unchecked($ptr.as_ptr() as *mut $ty) }};
}

pub mod allocs;
pub mod buf;
#[cfg(target_os = "linux")]
pub mod linux;
