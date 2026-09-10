//! Error handling for CEL runtime.
//!
//! `CelError` is the standard error type for all internal (Layer 2) runtime
//! functions. It is an alias for [`ferricel_types::CelRuntimeError`]. The
//! ABI boundary (Layer 1, `extern "C"`) converts it to a `CelValue::Error`
//! heap allocation before returning to Wasm callers.
//!
//! When a runtime error occurs (divide by zero, overflow, out of bounds, etc.),
//! the guest runtime calls `cel_abort` with the JSON-encoded error. The host
//! stops execution and returns the error to its caller.

/// The error type returned by all internal (Layer 2) runtime functions.
///
/// At the ABI boundary the wrapper converts this to `CelValue::Error(err)`.
pub use ferricel_types::CelRuntimeError as CelError;

/// Convert a `CelError` to a heap-allocated `CelValue::Error`, consuming it.
pub fn into_cel_value(err: CelError) -> crate::types::CelValue {
    crate::types::CelValue::Error(err)
}

/// Convenience alias for `Result<T, CelError>`.
pub type CelResult<T> = Result<T, CelError>;

/// Consume a `CelResult<CelValue>` and box it into a raw pointer for the ABI.
///
/// On `Ok(v)` → `Box::into_raw(Box::new(v))`
/// On `Err(e)` → `Box::into_raw(Box::new(CelValue::Error(e)))`
pub fn into_raw_result(r: CelResult<crate::types::CelValue>) -> *mut crate::types::CelValue {
    Box::into_raw(Box::new(match r {
        Ok(v) => v,
        Err(e) => into_cel_value(e),
    }))
}

// This function never returns - it terminates Wasm execution.
//
// Arguments:
// * `packed` - Packed i64 containing pointer (lower 32 bits) and length (upper 32 bits)
//              of the JSON-encoded `CelRuntimeError` in Wasm memory
//
// Only available when compiling to Wasm target
#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    pub fn cel_abort(packed: i64) -> !;
}

/// Abort execution with a structured error.
///
/// This function:
/// 1. Serializes the error to JSON
/// 2. Packs the pointer and length into an i64: pointer in low 32 bits,
///    length in high 32 bits
/// 3. Calls the host's `cel_abort` function, which terminates execution
///
/// # Note
/// This function never returns - execution is terminated by the host.
#[cfg(target_arch = "wasm32")]
pub fn abort_with_cel_error(error: &CelError) -> ! {
    // `CelRuntimeError` holds only strings, so serialization cannot fail.
    let json = serde_json::to_vec(error).expect("serializing CelRuntimeError never fails");
    // `json` must stay alive until the host has read it. The host never
    // returns from `cel_abort`, so leaking the buffer is correct.
    let json = json.leak();

    let ptr = json.as_ptr() as u64;
    let len = json.len() as u64;

    // Pack: low 32 bits = pointer, high 32 bits = length
    // Consistent with encode_ptr_len convention used elsewhere.
    let packed = ((len & 0xFFFFFFFF) << 32) | (ptr & 0xFFFFFFFF);

    unsafe { cel_abort(packed as i64) }
}

/// Test/mock version of `abort_with_cel_error` for non-Wasm targets.
///
/// There is no host on a native target, so this function panics. The panic
/// message is the `Display` text of the error, followed by the extension
/// origin when there is one.
#[cfg(not(target_arch = "wasm32"))]
pub fn abort_with_cel_error(error: &CelError) -> ! {
    match &error.origin {
        Some(origin) => panic!("{error} (from extension {origin})"),
        None => panic!("{error}"),
    }
}

/// Abort execution with an error message.
///
/// This is a shortcut for [`abort_with_cel_error`] with an error that has
/// no extension origin.
///
/// # Note
/// This function never returns - execution is terminated by the host.
pub fn abort_with_error(message: &str) -> ! {
    abort_with_cel_error(&CelError::new(message))
}

/// Abort Wasm execution if `ptr` points to a `CelValue::Error`; no-op otherwise.
///
/// This is the point where a CEL error value stops being a value that flows
/// through the expression tree (and may still be absorbed by `||`, `&&`, `?:`)
/// and becomes a hard failure reported to the host via `cel_abort`.
///
/// Callers:
/// - `cel_serialize_result`, before serializing the final result of a plain
///   CEL module.
/// - The VAP orchestrator, after evaluating each `matchCondition` and
///   `validation`, so a runtime error is never mistaken for a non-`false`
///   (passing) result.
///
/// The pointer is not consumed. A null pointer is a no-op.
///
/// # Safety
///
/// `ptr` must be null or a valid `CelValue` pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_abort_if_error(ptr: *mut crate::types::CelValue) {
    if ptr.is_null() {
        return;
    }
    if let crate::types::CelValue::Error(err) = unsafe { &*ptr } {
        abort_with_cel_error(err);
    }
}

/// Convenience macro for aborting with an error message.
///
/// # Examples
/// ```ignore
/// if denominator == 0 {
///     cel_abort!("division by zero");
/// }
/// ```
#[macro_export]
macro_rules! cel_abort {
    ($msg:expr) => {
        $crate::error::abort_with_error($msg)
    };
}

/// Helper function to create a CelValue::Error from a static string.
/// This is more convenient than cel_create_error when the message is already in Rust.
///
/// # Arguments
/// * `message` - The error message
///
/// # Returns
/// * Pointer to a heap-allocated CelValue::Error
pub fn create_error_value(message: &str) -> *mut crate::types::CelValue {
    Box::into_raw(Box::new(crate::types::CelValue::Error(CelError::new(
        message,
    ))))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{test_helpers::make_val, types::CelValue};

    #[test]
    fn abort_if_error_is_noop_for_non_error_values() {
        unsafe {
            cel_abort_if_error(std::ptr::null_mut());
            cel_abort_if_error(make_val(CelValue::Bool(true)));
            cel_abort_if_error(make_val(CelValue::Bool(false)));
            cel_abort_if_error(make_val(CelValue::Null));
            cel_abort_if_error(make_val(CelValue::String("x".into())));
        }
    }

    // The abort path (`CelValue::Error` → `cel_abort`) cannot be unit-tested
    // here: `abort_with_error` panics on native targets and a panic cannot
    // unwind through an `extern "C"` boundary. It is covered end-to-end by the
    // ferricel-core integration tests (plain CEL `1 / 0` and the VAP
    // runtime-error tests).
}
