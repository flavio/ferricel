//! Error handling for CEL runtime.
//!
//! `CelError` is the standard error type for all internal (Layer 2) runtime
//! functions. The ABI boundary (Layer 1, `extern "C"`) converts it to a
//! `CelValue::Error` heap allocation before returning to Wasm callers.
//!
//! When a runtime error occurs (divide by zero, overflow, out of bounds, etc.),
//! the guest runtime calls cel_abort which terminates execution and returns
//! the error to the host.

/// The error type returned by all internal (Layer 2) runtime functions.
///
/// At the ABI boundary the wrapper converts this to `CelValue::Error(msg)`.
#[derive(Debug, Clone, PartialEq)]
pub struct CelError(pub String);

impl CelError {
    pub fn new(msg: impl Into<String>) -> Self {
        CelError(msg.into())
    }

    /// Convert to a heap-allocated `CelValue::Error`, consuming `self`.
    pub fn into_cel_value(self) -> crate::types::CelValue {
        crate::types::CelValue::Error(self.0)
    }
}

impl std::fmt::Display for CelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Convenience alias for `Result<T, CelError>`.
pub type CelResult<T> = Result<T, CelError>;

/// Consume a `CelResult<CelValue>` and box it into a raw pointer for the ABI.
///
/// On `Ok(v)` → `Box::into_raw(Box::new(v))`
/// On `Err(e)` → `Box::into_raw(Box::new(CelValue::Error(e.0)))`
pub fn into_raw_result(r: CelResult<crate::types::CelValue>) -> *mut crate::types::CelValue {
    Box::into_raw(Box::new(match r {
        Ok(v) => v,
        Err(e) => e.into_cel_value(),
    }))
}

// This function never returns - it terminates Wasm execution.
//
// Arguments:
// * `packed` - Packed i64 containing pointer (lower 32 bits) and length (upper 32 bits)
//              of the error message string in Wasm memory
//
// Only available when compiling to Wasm target
#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    pub fn cel_abort(packed: i64) -> !;
}

/// Abort execution with an error message.
///
/// This function:
/// 1. Gets the pointer and length of the error message
/// 2. Packs them into an i64: pointer in low 32 bits, length in high 32 bits
/// 3. Calls the host's cel_abort function which terminates execution
///
/// # Arguments
/// * `message` - The error message to report
///
/// # Note
/// This function never returns - execution is terminated by the host.
#[cfg(target_arch = "wasm32")]
pub fn abort_with_error(message: &str) -> ! {
    let ptr = message.as_ptr() as u64;
    let len = message.len() as u64;

    // Pack: low 32 bits = pointer, high 32 bits = length
    // Consistent with encode_ptr_len convention used elsewhere.
    let packed = ((len & 0xFFFFFFFF) << 32) | (ptr & 0xFFFFFFFF);

    unsafe { cel_abort(packed as i64) }
}

/// Test/mock version of abort_with_error for non-Wasm targets.
/// Just panics with the error message.
#[cfg(not(target_arch = "wasm32"))]
pub fn abort_with_error(message: &str) -> ! {
    panic!("{}", message);
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
    Box::into_raw(Box::new(crate::types::CelValue::Error(message.to_string())))
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
