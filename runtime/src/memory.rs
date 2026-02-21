//! Memory allocation and deallocation for WASM linear memory.

use std::mem;

extern crate alloc;
use alloc::vec::Vec;

/// Allocates memory in WASM linear memory.
/// Returns a pointer to the allocated buffer.
#[unsafe(no_mangle)]
pub extern "C" fn cel_malloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    ptr
}

/// Deallocates memory previously allocated with cel_malloc.
#[unsafe(no_mangle)]
pub extern "C" fn cel_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len, len);
            // Vec will be dropped here, freeing the memory
        }
    }
}
