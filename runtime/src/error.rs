//! Error handling for CEL runtime.
//!
//! When a runtime error occurs (divide by zero, overflow, out of bounds, etc.),
//! the guest runtime calls cel_abort which terminates execution and returns
//! the error to the host.

// External function provided by WASM host for error handling.
// This function never returns - it terminates WASM execution.
//
// Arguments:
// * `packed` - Packed i64 containing address (upper 32 bits) and length (lower 32 bits)
//              of the error message string in WASM memory
//
// Only available when compiling to WASM target
#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    pub fn cel_abort(packed: i64) -> !;
}

/// Abort execution with an error message.
///
/// This function:
/// 1. Gets the pointer and length of the error message
/// 2. Packs them into an i64: (address << 32) | length
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

    // Pack: upper 32 bits = address, lower 32 bits = length
    // This matches the unpacking logic in the host's cel_abort
    let packed = ((ptr & 0xFFFFFFFF) << 32) | (len & 0xFFFFFFFF);

    unsafe { cel_abort(packed as i64) }
}

/// Test/mock version of abort_with_error for non-WASM targets.
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
