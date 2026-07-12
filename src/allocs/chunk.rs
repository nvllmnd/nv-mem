use core::{cell::UnsafeCell, ptr::NonNull};

pub struct ChunkAlloc {
    ptr: NonNull<UnsafeCell<[u8]>>,
}
