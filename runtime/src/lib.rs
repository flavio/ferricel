use std::mem;

// 1. Memory Allocator for the Host
#[unsafe(no_mangle)]
pub extern "C" fn cel_malloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    ptr
}

// 2. Helper function for CEL logic (Exported so Walrus can find it)
#[unsafe(no_mangle)]
pub extern "C" fn cel_int_add(a: i64, b: i64) -> i64 {
    a + b
}
