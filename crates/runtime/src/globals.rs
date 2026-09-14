//! Global variable storage for CEL expression evaluation.
//!
//! All runtime variables are stored in a single Map value which can contain
//! any user-provided bindings (e.g., `object`, `oldObject`, `request` for K8s,
//! or custom variables for general CEL expressions).

use std::{cell::UnsafeCell, collections::HashMap};

use crate::{
    error::{CelError, abort_with_error},
    types::{CelMapKey, CelValue},
};

/// Wrapper that lets us keep an owned, mutable global in a `static` without
/// going through the (edition-2024-forbidden) `static mut` reference form.
///
/// # Safety
/// The Wasm guest runtime executes on a single thread, so unsynchronized
/// access to the wrapped value can never race.
struct GlobalCell<T>(UnsafeCell<T>);

// SAFETY: see struct doc comment above.
unsafe impl<T> Sync for GlobalCell<T> {}

/// Global storage for all variable bindings, as an owned `CelValue::Object`.
///
/// Initialized by [`cel_init_bindings`] before expression evaluation. The
/// runtime *owns* this value: once handed to `cel_init_bindings`, the caller
/// must not free it independently. It is dropped when the module instance is
/// torn down (or replaced by a subsequent `cel_init_bindings` call).
static BINDINGS: GlobalCell<Option<Box<CelValue>>> = GlobalCell(UnsafeCell::new(None));

/// Initialize the global bindings map.
///
/// Takes ownership of `ptr`: the runtime becomes responsible for the pointee
/// and the caller must not free it afterwards. Any previously stored
/// bindings are dropped.
///
/// # Parameters
/// - `ptr`: Pointer to a boxed CelValue::Map (from cel_deserialize_json)
///   - Should be a Map with variable names as string keys
///   - Can be null to use an empty map
///
/// # Safety
/// - Must be called before any expression evaluation
/// - `ptr` must be a valid, uniquely-owned pointer from cel_deserialize_json
///   (or a compatible allocator), or null
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_init_bindings(ptr: *mut CelValue) {
    unsafe {
        let new_value = if ptr.is_null() {
            None
        } else {
            Some(Box::from_raw(ptr))
        };
        *BINDINGS.0.get() = new_value;
    }
}

/// Borrow the bindings map, if bindings are initialized and hold an Object.
///
/// # Safety
/// Must only be called from the single-threaded Wasm guest environment.
unsafe fn bindings_map() -> Option<&'static HashMap<CelMapKey, CelValue>> {
    unsafe {
        match &*BINDINGS.0.get() {
            Some(boxed) => match boxed.as_ref() {
                CelValue::Object(m) => Some(m),
                _ => None,
            },
            None => None,
        }
    }
}

/// Mutably borrow the bindings map, if bindings are initialized and hold an
/// Object.
///
/// # Safety
/// Must only be called from the single-threaded Wasm guest environment.
unsafe fn bindings_map_mut() -> Option<&'static mut HashMap<CelMapKey, CelValue>> {
    unsafe {
        match &mut *BINDINGS.0.get() {
            Some(boxed) => match boxed.as_mut() {
                CelValue::Object(m) => Some(m),
                _ => None,
            },
            None => None,
        }
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
/// - The returned pointer is a freshly-allocated clone owned by the caller;
///   it stays valid independently of the global bindings' lifetime
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_get_variable(name_ptr: *const u8, name_len: i32) -> *mut CelValue {
    unsafe {
        // Get the bindings map, if initialized and shaped as an Object.
        let Some(map) = bindings_map() else {
            return std::ptr::null_mut();
        };

        // Read the variable name from Wasm memory
        let name_slice = std::slice::from_raw_parts(name_ptr, name_len as usize);
        let name = match std::str::from_utf8(name_slice) {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };

        // Look up the variable in the map using CelMapKey
        let key = CelMapKey::String(name.to_string());

        match map.get(&key) {
            Some(value) => Box::into_raw(Box::new(value.clone())),
            None => std::ptr::null_mut(),
        }
    }
}

/// Insert or update a named variable in the global bindings map.
///
/// This is used by the VAP compiler to incrementally build the `variables` map:
/// after evaluating each variable expression the result is stored here so
/// subsequent expressions can reference it via `variables.<name>`.
///
/// # Parameters
/// - `name_ptr`: Pointer to UTF-8 string containing the variable name
/// - `name_len`: Length of the variable name in bytes
/// - `value_ptr`: Pointer to a boxed CelValue to store (must be non-null)
///
/// # Safety
/// - Must be called after `cel_init_bindings`
/// - `value_ptr` must be a valid, non-null CelValue pointer
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_set_variable(
    name_ptr: *const u8,
    name_len: i32,
    value_ptr: *mut CelValue,
) {
    unsafe {
        if (*BINDINGS.0.get()).is_none() {
            abort_with_error("cel_set_variable called before cel_init_bindings");
        }

        let name_slice = std::slice::from_raw_parts(name_ptr, name_len as usize);
        let name = match std::str::from_utf8(name_slice) {
            Ok(s) => s,
            Err(_) => abort_with_error("cel_set_variable: variable name is not valid UTF-8"),
        };

        let key = CelMapKey::String(name.to_string());
        let value = if value_ptr.is_null() {
            CelValue::Null
        } else {
            (*value_ptr).clone()
        };

        // Bindings are initialized (checked above); if they are not shaped as
        // an Object, silently ignore the write (mirrors `cel_get_variable`'s
        // behavior for non-Object bindings).
        if let Some(map) = bindings_map_mut() {
            map.insert(key, value);
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
        Box::into_raw(Box::new(CelValue::Error(CelError::new(msg))))
    }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    #[test]
    #[serial]
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

            let value = Box::from_raw(var_ptr);
            assert!(matches!(*value, CelValue::Int(42)));

            // NOTE: `cel_init_bindings` took ownership of `ptr`; it is owned
            // and will be dropped by `BINDINGS` (or replaced by the next
            // test's `cel_init_bindings` call). Do not free it here.
        }
    }

    #[test]
    #[serial]
    fn test_variable_not_found() {
        let map = std::collections::HashMap::new();
        let bindings = Box::new(CelValue::Object(map));
        let ptr = Box::into_raw(bindings);

        unsafe {
            cel_init_bindings(ptr);

            let name = b"nonexistent";
            let var_ptr = cel_get_variable(name.as_ptr(), name.len() as i32);
            assert!(var_ptr.is_null());
        }
    }

    #[test]
    #[serial]
    fn test_null_bindings() {
        unsafe {
            cel_init_bindings(std::ptr::null_mut());

            let name = b"x";
            let var_ptr = cel_get_variable(name.as_ptr(), name.len() as i32);
            assert!(var_ptr.is_null());
        }
    }

    #[test]
    #[serial]
    fn test_set_variable() {
        let map = std::collections::HashMap::new();
        let bindings = Box::new(CelValue::Object(map));
        let ptr = Box::into_raw(bindings);

        unsafe {
            cel_init_bindings(ptr);

            let name = b"myvar";
            let value = Box::into_raw(Box::new(CelValue::Int(99)));
            cel_set_variable(name.as_ptr(), name.len() as i32, value);

            let var_ptr = cel_get_variable(name.as_ptr(), name.len() as i32);
            assert!(!var_ptr.is_null());
            let got = Box::from_raw(var_ptr);
            assert!(matches!(*got, CelValue::Int(99)));

            // Cleanup: `value` was cloned by `cel_set_variable`, so it is
            // still owned by this test and must be freed here. `ptr` is
            // owned by `BINDINGS` and must not be freed.
            let _ = Box::from_raw(value);
        }
    }

    #[test]
    #[serial]
    fn test_reinit_replaces_bindings() {
        let mut map1 = std::collections::HashMap::new();
        map1.insert(CelMapKey::String("x".to_string()), CelValue::Int(1));
        let ptr1 = Box::into_raw(Box::new(CelValue::Object(map1)));

        let mut map2 = std::collections::HashMap::new();
        map2.insert(CelMapKey::String("x".to_string()), CelValue::Int(2));
        let ptr2 = Box::into_raw(Box::new(CelValue::Object(map2)));

        unsafe {
            cel_init_bindings(ptr1);
            // Re-initializing drops the previous bindings and takes
            // ownership of the new ones; `ptr1` must not be freed separately.
            cel_init_bindings(ptr2);

            let name = b"x";
            let var_ptr = cel_get_variable(name.as_ptr(), name.len() as i32);
            assert!(!var_ptr.is_null());
            let got = Box::from_raw(var_ptr);
            assert!(matches!(*got, CelValue::Int(2)));
        }
    }
}
