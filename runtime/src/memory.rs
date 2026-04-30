//! Memory allocation for WASM linear memory.
//!
//! Under the arena allocator (`lol_alloc::LeakingAllocator`), all allocations
//! are bump-pointer advances and `dealloc` is a no-op. Memory is released when
//! the host drops the WASM instance.

use std::mem;

/// Allocates memory in WASM linear memory.
/// Returns a pointer to the allocated buffer.
///
/// # Safety
///
/// This function is unsafe because it returns a raw pointer. The caller must ensure:
/// - The returned pointer is only used within the lifetime of the WASM instance
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub extern "C" fn cel_malloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    ptr
}
