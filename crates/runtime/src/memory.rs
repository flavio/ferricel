//! Memory allocation for Wasm linear memory, and reading values back out of it.
//!
//! Under the arena allocator (`lol_alloc::LeakingAllocator`), all allocations
//! are bump-pointer advances and `dealloc` is a no-op. Memory is released when
//! the host drops the Wasm instance.

use std::mem;

use crate::error::abort_with_error;

/// Allocates memory in Wasm linear memory.
/// Returns a pointer to the allocated buffer.
///
/// # Safety
///
/// This function is unsafe because it returns a raw pointer. The caller must ensure:
/// - The returned pointer is only used within the lifetime of the Wasm instance
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub extern "C" fn cel_malloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    ptr
}

/// Read a `CelValue` from a raw pointer, aborting hard if null.
///
/// Reads a `CelValue` out of a raw pointer, aborting on null.
///
/// A null pointer reaching an operator is a compiler or runtime bug — since
/// `cel_get_variable` now returns a `CelValue::Error` (never null) for unbound
/// variables, null should never appear here. If it does, abort loudly instead
/// of silently producing a wrong error value.
///
/// Under the arena allocator (`lol_alloc::LeakingAllocator`) dealloc is a no-op,
/// so `ptr::read` is used to bitwise-move the value out of arena memory without
/// cloning or freeing.
///
/// # Safety
/// `ptr` must point to a valid, aligned `CelValue` in live memory.
#[inline]
pub unsafe fn read_ptr(ptr: *mut crate::types::CelValue) -> crate::types::CelValue {
    if ptr.is_null() {
        abort_with_error("null CelValue pointer: this is a compiler or runtime bug");
    }
    unsafe { std::ptr::read(ptr) }
}

/// Read a `&str` from a raw pointer and length in Wasm linear memory.
///
/// A zero (or negative) length gives `""` without a memory read, because the
/// compiler passes a null pointer for an empty string. Invalid UTF-8 aborts:
/// this can only happen from a compiler bug or memory corruption, since every
/// caller compiles the bytes from a Rust `&str`.
///
/// # Safety
/// When `len > 0`, `ptr` must point to `len` valid, initialized UTF-8 bytes.
#[inline]
pub unsafe fn read_str<'a>(ptr: *const u8, len: i32) -> &'a str {
    if len <= 0 {
        return "";
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    std::str::from_utf8(bytes)
        .unwrap_or_else(|_| abort_with_error("string argument is not valid UTF-8"))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::zero_length(0)]
    #[case::negative_length(-1)]
    fn read_str_empty_length_is_empty_str_without_reading_the_pointer(#[case] len: i32) {
        // A null pointer with a non-positive length must not be read.
        let s = unsafe { read_str(std::ptr::null(), len) };
        assert_eq!(s, "");
    }

    #[test]
    fn read_str_reads_valid_utf8() {
        let bytes = "validation[0]".as_bytes();
        let s = unsafe { read_str(bytes.as_ptr(), bytes.len() as i32) };
        assert_eq!(s, "validation[0]");
    }

    // Invalid UTF-8, and the null-pointer abort in `read_ptr`, cannot be
    // unit-tested here: `abort_with_error` panics on native targets and a
    // panic cannot unwind through an `extern "C"` boundary.
}
