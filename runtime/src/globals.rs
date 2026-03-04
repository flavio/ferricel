//! Global variable storage for `input` and `data` during CEL expression evaluation.
//! These globals are initialized once per `validate` call and live for the duration
//! of the expression evaluation.

use crate::types::CelValue;
use std::ptr;

/// Global storage for the `input` variable
/// Initialized by validate() before expression evaluation
static mut INPUT_VALUE: *mut CelValue = ptr::null_mut();

/// Global storage for the `data` variable
/// Initialized by validate() before expression evaluation
static mut DATA_VALUE: *mut CelValue = ptr::null_mut();

/// Initialize the global `input` variable.
///
/// # Parameters
/// - `ptr`: Pointer to a boxed CelValue (from cel_deserialize_json)
///   - Can be null if input is not provided
///
/// # Safety
/// - Must be called before any expression evaluation that uses `input`
/// - `ptr` must be a valid pointer from cel_deserialize_json or null
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_init_input(ptr: *mut CelValue) {
    // SAFETY: Writing to static mut, single-threaded WASM environment
    unsafe {
        INPUT_VALUE = ptr;
    }
}

/// Initialize the global `data` variable.
///
/// # Parameters
/// - `ptr`: Pointer to a boxed CelValue (from cel_deserialize_json)
///   - Can be null if data is not provided
///
/// # Safety
/// - Must be called before any expression evaluation that uses `data`
/// - `ptr` must be a valid pointer from cel_deserialize_json or null
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_init_data(ptr: *mut CelValue) {
    // SAFETY: Writing to static mut, single-threaded WASM environment
    unsafe {
        DATA_VALUE = ptr;
    }
}

/// Get the global `input` variable.
///
/// # Returns
/// - Pointer to the input CelValue
/// - Null pointer if input was not initialized or is absent
///
/// # Panics
/// - Panics if called before cel_init_input
///
/// # Safety
/// - Safe to call after cel_init_input in single-threaded WASM environment
/// - Returned pointer is valid until cel_reset_globals is called
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_get_input() -> *mut CelValue {
    // SAFETY: Reading from static, single-threaded WASM environment
    unsafe { INPUT_VALUE }
}

/// Get the global `data` variable.
///
/// # Returns
/// - Pointer to the data CelValue
/// - Null pointer if data was not initialized or is absent
///
/// # Panics
/// - Panics if called before cel_init_data
///
/// # Safety
/// - Safe to call after cel_init_data in single-threaded WASM environment
/// - Returned pointer is valid until cel_reset_globals is called
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_get_data() -> *mut CelValue {
    // SAFETY: Reading from static, single-threaded WASM environment
    unsafe { DATA_VALUE }
}

/// Reset global variables to null.
/// Useful for cleanup between evaluations (though not strictly necessary
/// since WASM instances are short-lived).
///
/// # Safety
/// - Safe to call at any time
/// - Does not free the pointed-to values (caller must call cel_free_value separately)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_reset_globals() {
    // SAFETY: Writing to static mut, single-threaded WASM environment
    unsafe {
        INPUT_VALUE = ptr::null_mut();
        DATA_VALUE = ptr::null_mut();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_and_get_input() {
        let value = Box::new(CelValue::Int(42));
        let ptr = Box::into_raw(value);

        unsafe {
            cel_init_input(ptr);
            let retrieved = cel_get_input();
            assert_eq!(retrieved, ptr);

            // Cleanup
            let _boxed = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_init_and_get_data() {
        let value = Box::new(CelValue::Bool(true));
        let ptr = Box::into_raw(value);

        unsafe {
            cel_init_data(ptr);
            let retrieved = cel_get_data();
            assert_eq!(retrieved, ptr);

            // Cleanup
            let _boxed = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_null_input() {
        unsafe {
            cel_init_input(ptr::null_mut());
            let retrieved = cel_get_input();
            assert!(retrieved.is_null());
        }
    }

    #[test]
    fn test_null_data() {
        unsafe {
            cel_init_data(ptr::null_mut());
            let retrieved = cel_get_data();
            assert!(retrieved.is_null());
        }
    }

    #[test]
    fn test_reset_globals() {
        let value = Box::new(CelValue::Int(99));
        let ptr = Box::into_raw(value);

        unsafe {
            cel_init_input(ptr);
            cel_init_data(ptr);

            assert!(!cel_get_input().is_null());
            assert!(!cel_get_data().is_null());

            cel_reset_globals();

            assert!(cel_get_input().is_null());
            assert!(cel_get_data().is_null());

            // Cleanup
            let _boxed = Box::from_raw(ptr);
        }
    }
}
