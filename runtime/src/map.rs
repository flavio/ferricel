//! Map operations for CEL Object (HashMap) types.
//!
//! Provides functions for creating and populating map literals, and for
//! iterating over maps (keys extraction, value lookup).

use crate::error::abort_with_error;
use crate::types::{CelMapKey, CelValue};
use slog::{debug, error};
use std::collections::HashMap;

/// Create an empty CelValue map (Object).
///
/// # Safety
///
/// The returned pointer must be freed using the appropriate cleanup function.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_create_map() -> *mut CelValue {
    let map = CelValue::Object(HashMap::new());
    Box::into_raw(Box::new(map))
}

/// Insert a key-value pair into a CelValue map (mutates in place).
///
/// # Safety
///
/// All pointer arguments must be valid, aligned, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_map_insert(
    map_ptr: *mut CelValue,
    key_ptr: *mut CelValue,
    value_ptr: *mut CelValue,
) {
    let log = crate::logging::get_logger();

    if map_ptr.is_null() {
        error!(log, "Map pointer is null"; "function" => "cel_map_insert", "parameter" => "map_ptr");
        abort_with_error("no such overload");
    }
    if key_ptr.is_null() {
        error!(log, "Key pointer is null"; "function" => "cel_map_insert", "parameter" => "key_ptr");
        abort_with_error("no such overload");
    }
    if value_ptr.is_null() {
        error!(log, "Value pointer is null"; "function" => "cel_map_insert", "parameter" => "value_ptr");
        abort_with_error("no such overload");
    }

    let map_value = unsafe { &mut *map_ptr };
    let key = unsafe { &*key_ptr };
    let value = unsafe { &*value_ptr };

    let map_key = match CelMapKey::from_cel_value(key) {
        Some(k) => k,
        None => {
            error!(log, "Map key must be bool, int, uint, or string";
                "function" => "cel_map_insert",
                "actual_key_type" => format!("{:?}", key));
            abort_with_error("no such overload")
        }
    };

    match map_value {
        CelValue::Object(hash_map) => {
            debug!(log, "Inserting into map";
                "key" => map_key.to_string_key(),
                "current_size" => hash_map.len());
            hash_map.insert(map_key, value.clone());
        }
        _ => {
            error!(log, "Type mismatch in map operation";
                "function" => "cel_map_insert",
                "expected" => "Object",
                "actual" => format!("{:?}", map_value));
            abort_with_error("no such overload")
        }
    }
}

/// Return the keys of a CelValue map as a new CelValue::Array.
///
/// The order of keys matches HashMap iteration order (unspecified).
///
/// # Safety
///
/// `map_ptr` must be a valid non-null pointer to a CelValue::Object.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_map_keys(map_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if map_ptr.is_null() {
        error!(log, "Map pointer is null"; "function" => "cel_map_keys");
        abort_with_error("no such overload");
    }

    let map_value = unsafe { &*map_ptr };

    match map_value {
        CelValue::Object(hash_map) => {
            let keys: Vec<CelValue> = hash_map
                .keys()
                .map(|k| match k {
                    CelMapKey::Bool(b) => CelValue::Bool(*b),
                    CelMapKey::Int(i) => CelValue::Int(*i),
                    CelMapKey::UInt(u) => CelValue::UInt(*u),
                    CelMapKey::String(s) => CelValue::String(s.clone()),
                })
                .collect();
            Box::into_raw(Box::new(CelValue::Array(keys)))
        }
        _ => {
            error!(log, "Type mismatch: expected map";
                "function" => "cel_map_keys",
                "actual" => format!("{:?}", map_value));
            abort_with_error("no such overload")
        }
    }
}

/// Get a value from a CelValue map by key.
///
/// Returns a clone of the associated value, or `CelValue::Error("no such key")` if absent.
///
/// # Safety
///
/// Both `map_ptr` and `key_ptr` must be valid non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_map_get(
    map_ptr: *mut CelValue,
    key_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if map_ptr.is_null() {
        error!(log, "Map pointer is null"; "function" => "cel_map_get");
        abort_with_error("no such overload");
    }
    if key_ptr.is_null() {
        error!(log, "Key pointer is null"; "function" => "cel_map_get");
        abort_with_error("no such overload");
    }

    let map_value = unsafe { &*map_ptr };
    let key = unsafe { &*key_ptr };

    let map_key = match CelMapKey::from_cel_value(key) {
        Some(k) => k,
        None => {
            error!(log, "Key type not valid for map lookup"; "function" => "cel_map_get");
            return crate::error::create_error_value("no such key");
        }
    };

    match map_value {
        CelValue::Object(hash_map) => match hash_map.get(&map_key) {
            Some(v) => Box::into_raw(Box::new(v.clone())),
            None => crate::error::create_error_value("no such key"),
        },
        _ => {
            error!(log, "Type mismatch: expected map";
                "function" => "cel_map_get",
                "actual" => format!("{:?}", map_value));
            abort_with_error("no such overload")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{make_int, make_str, make_val, read_val};
    use rstest::rstest;

    #[test]
    fn test_create_empty_map() {
        let map_ptr = unsafe { cel_create_map() };
        match unsafe { &*map_ptr } {
            CelValue::Object(m) => assert!(m.is_empty()),
            _ => panic!("Expected Object"),
        }
        unsafe {
            let _ = Box::from_raw(map_ptr);
        }
    }

    #[test]
    fn test_map_insert_single_entry() {
        let map_ptr = unsafe { cel_create_map() };
        let key_ptr = Box::into_raw(Box::new(CelValue::String("name".to_string())));
        let value_ptr = Box::into_raw(Box::new(CelValue::String("Alice".to_string())));
        unsafe { cel_map_insert(map_ptr, key_ptr, value_ptr) };

        match unsafe { &*map_ptr } {
            CelValue::Object(m) => {
                assert_eq!(m.len(), 1);
                assert_eq!(
                    m.get(&CelMapKey::String("name".to_string())),
                    Some(&CelValue::String("Alice".to_string()))
                );
            }
            _ => panic!("Expected Object"),
        }
        unsafe {
            let _ = Box::from_raw(map_ptr);
            let _ = Box::from_raw(key_ptr);
            let _ = Box::from_raw(value_ptr);
        }
    }

    #[test]
    fn test_map_insert_multiple_entries() {
        let map_ptr = unsafe { cel_create_map() };
        let k1 = Box::into_raw(Box::new(CelValue::String("name".to_string())));
        let v1 = Box::into_raw(Box::new(CelValue::String("Alice".to_string())));
        unsafe { cel_map_insert(map_ptr, k1, v1) };
        let k2 = Box::into_raw(Box::new(CelValue::String("age".to_string())));
        let v2 = Box::into_raw(Box::new(CelValue::Int(30)));
        unsafe { cel_map_insert(map_ptr, k2, v2) };

        match unsafe { &*map_ptr } {
            CelValue::Object(m) => {
                assert_eq!(m.len(), 2);
                assert_eq!(
                    m.get(&CelMapKey::String("name".to_string())),
                    Some(&CelValue::String("Alice".to_string()))
                );
                assert_eq!(
                    m.get(&CelMapKey::String("age".to_string())),
                    Some(&CelValue::Int(30))
                );
            }
            _ => panic!("Expected Object"),
        }
        unsafe {
            let _ = Box::from_raw(map_ptr);
            let _ = Box::from_raw(k1);
            let _ = Box::from_raw(v1);
            let _ = Box::from_raw(k2);
            let _ = Box::from_raw(v2);
        }
    }

    #[rstest]
    #[case("key1", CelValue::Int(1))]
    #[case("key2", CelValue::String("hello".to_string()))]
    fn test_map_keys_string_keys(#[case] key: &str, #[case] val: CelValue) {
        let map_ptr = unsafe { cel_create_map() };
        let key_ptr = make_str(key);
        let val_ptr = make_val(val);
        unsafe { cel_map_insert(map_ptr, key_ptr, val_ptr) };

        let keys_ptr = unsafe { cel_map_keys(map_ptr) };
        let keys = read_val(keys_ptr);

        match keys {
            CelValue::Array(arr) => {
                assert_eq!(arr.len(), 1);
                assert_eq!(arr[0], CelValue::String(key.to_string()));
            }
            _ => panic!("Expected Array, got {:?}", keys),
        }
        unsafe {
            let _ = Box::from_raw(map_ptr);
            let _ = Box::from_raw(key_ptr);
            let _ = Box::from_raw(val_ptr);
        }
    }

    #[test]
    fn test_map_keys_empty() {
        let map_ptr = make_val(CelValue::Object(HashMap::new()));
        let keys_ptr = unsafe { cel_map_keys(map_ptr) };
        let keys = read_val(keys_ptr);
        match keys {
            CelValue::Array(arr) => assert!(arr.is_empty()),
            _ => panic!("Expected Array"),
        }
        unsafe {
            let _ = Box::from_raw(map_ptr);
        }
    }

    #[rstest]
    #[case("key1", CelValue::Int(42))]
    #[case("hello", CelValue::Bool(true))]
    fn test_map_get_string_key(#[case] key: &str, #[case] expected: CelValue) {
        let map_ptr = unsafe { cel_create_map() };
        let key_ptr = make_str(key);
        let val_ptr = make_val(expected.clone());
        unsafe { cel_map_insert(map_ptr, key_ptr, val_ptr) };

        let lookup_ptr = make_str(key);
        let result_ptr = unsafe { cel_map_get(map_ptr, lookup_ptr) };
        let result = read_val(result_ptr);

        assert_eq!(result, expected);
        unsafe {
            let _ = Box::from_raw(map_ptr);
            let _ = Box::from_raw(key_ptr);
            let _ = Box::from_raw(val_ptr);
            let _ = Box::from_raw(lookup_ptr);
        }
    }

    #[test]
    fn test_map_get_missing_key() {
        let map_ptr = unsafe { cel_create_map() };
        let key_ptr = make_str("missing");
        let result_ptr = unsafe { cel_map_get(map_ptr, key_ptr) };
        let result = read_val(result_ptr);
        assert!(matches!(result, CelValue::Error(_)));
        unsafe {
            let _ = Box::from_raw(map_ptr);
            let _ = Box::from_raw(key_ptr);
        }
    }

    #[test]
    fn test_map_get_int_key() {
        let map_ptr = unsafe { cel_create_map() };
        let key_ptr = make_int(7);
        let val_ptr = make_str("seven");
        unsafe { cel_map_insert(map_ptr, key_ptr, val_ptr) };

        let lookup_ptr = make_int(7);
        let result_ptr = unsafe { cel_map_get(map_ptr, lookup_ptr) };
        let result = read_val(result_ptr);
        assert_eq!(result, CelValue::String("seven".to_string()));
        unsafe {
            let _ = Box::from_raw(map_ptr);
            let _ = Box::from_raw(key_ptr);
            let _ = Box::from_raw(val_ptr);
            let _ = Box::from_raw(lookup_ptr);
        }
    }
}
