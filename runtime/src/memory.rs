//! Memory allocation and deallocation for WASM linear memory.

use std::mem;

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
///
/// # Safety
/// The pointer must have been allocated by `cel_malloc` with the same length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        // SAFETY: Caller ensures ptr was allocated by cel_malloc with the same length
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len, len);
            // Vec will be dropped here, freeing the memory
        }
    }
}
