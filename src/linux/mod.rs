pub mod valloc;
pub mod vmem;

pub fn page_size() -> usize {
    static mut SIZE: usize = 0;
    unsafe {
        if SIZE == 0 {
            SIZE = libc::sysconf(libc::_SC_PAGE_SIZE) as usize;
        }
        SIZE
    }
}
