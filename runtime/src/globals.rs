//! Global variable storage for CEL expression evaluation.
//!
//! All runtime variables are stored in a single Map value which can contain
//! any user-provided bindings (e.g., `object`, `oldObject`, `request` for K8s,
//! or custom variables for general CEL expressions).

use std::ptr;

use crate::types::{CelMapKey, CelValue};

/// Global storage for all variable bindings as a Map
/// Initialized by validate() before expression evaluation
static mut BINDINGS: *mut CelValue = ptr::null_mut();

/// Initialize the global bindings map.
///
/// # Parameters
/// - `ptr`: Pointer to a boxed CelValue::Map (from cel_deserialize_json)
///   - Should be a Map with variable names as string keys
///   - Can be null to use an empty map
///
/// # Safety
/// - Must be called before any expression evaluation
/// - `ptr` must be a valid pointer from cel_deserialize_json or null
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_init_bindings(ptr: *mut CelValue) {
    unsafe {
        BINDINGS = ptr;
    }
}

/// Get a variable value by name from the bindings map.
///
/// # Parameters
/// - `name_ptr`: Pointer to UTF-8 string containing variable name
/// - `name_len`: Length of variable name in bytes
///
/// # Returns
/// - Pointer to the CelValue for that variable
/// - Null pointer if variable not found or bindings not initialized
///
/// # Safety
/// - Safe to call after cel_init_bindings in single-threaded Wasm environment
/// - Returned pointer is valid until cel_reset_globals is called
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_get_variable(name_ptr: *const u8, name_len: i32) -> *mut CelValue {
    unsafe {
        // Check if bindings are initialized
        if BINDINGS.is_null() {
            return ptr::null_mut();
        }

        // Get the bindings value
        let bindings_ref = &*BINDINGS;

        // Extract the map from the CelValue
        let map = match bindings_ref {
            CelValue::Object(m) => m,
            _ => return ptr::null_mut(), // Bindings should be a map
        };

        // Read the variable name from Wasm memory
        let name_slice = std::slice::from_raw_parts(name_ptr, name_len as usize);
        let name = match std::str::from_utf8(name_slice) {
            Ok(s) => s,
            Err(_) => return ptr::null_mut(),
        };

        // Look up the variable in the map using CelMapKey
        let key = CelMapKey::String(name.to_string());

        match map.get(&key) {
            Some(value) => Box::into_raw(Box::new(value.clone())),
            None => ptr::null_mut(),
        }
    }
}

/// Return a `CelValue::Error("no such attribute: <name>")` for an unbound variable.
///
/// Called by the compiler after the full lookup chain finds no binding for a variable.
///
/// # Parameters
/// - `name_ptr`: Pointer to UTF-8 string containing the variable name
/// - `name_len`: Length of the variable name in bytes
///
/// # Returns
/// - Owned pointer to a `CelValue::Error` — never null
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_unbound_variable_error(
    name_ptr: *const u8,
    name_len: i32,
) -> *mut CelValue {
    unsafe {
        let name_slice = std::slice::from_raw_parts(name_ptr, name_len as usize);
        let name = std::str::from_utf8(name_slice).unwrap_or("<invalid utf-8>");
        let msg = format!("no such attribute: {name}");
        Box::into_raw(Box::new(CelValue::Error(msg)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_and_get_variable() {
        let mut map = std::collections::HashMap::new();
        map.insert(CelMapKey::String("x".to_string()), CelValue::Int(42));
        let bindings = Box::new(CelValue::Object(map));
        let ptr = Box::into_raw(bindings);

        unsafe {
            cel_init_bindings(ptr);

            let name = b"x";
            let var_ptr = cel_get_variable(name.as_ptr(), name.len() as i32);
            assert!(!var_ptr.is_null());

            let value = &*var_ptr;
            assert!(matches!(value, CelValue::Int(42)));

            // Cleanup
            let _boxed = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_variable_not_found() {
        let map = std::collections::HashMap::new();
        let bindings = Box::new(CelValue::Object(map));
        let ptr = Box::into_raw(bindings);

        unsafe {
            cel_init_bindings(ptr);

            let name = b"nonexistent";
            let var_ptr = cel_get_variable(name.as_ptr(), name.len() as i32);
            assert!(var_ptr.is_null());

            // Cleanup
            let _boxed = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_null_bindings() {
        unsafe {
            cel_init_bindings(ptr::null_mut());

            let name = b"x";
            let var_ptr = cel_get_variable(name.as_ptr(), name.len() as i32);
            assert!(var_ptr.is_null());
        }
    }
}
