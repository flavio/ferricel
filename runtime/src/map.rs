//! Map operations for CEL Object (HashMap) types.
//!
//! Provides functions for creating and populating map literals.

use crate::error::abort_with_error;
use crate::types::CelValue;
use slog::{debug, error};
use std::collections::HashMap;

/// Create an empty CelValue map (Object).
///
/// # Returns
/// - Pointer to a new CelValue::Object (empty HashMap)
///
/// # Safety
/// - Returns a heap-allocated pointer that must be properly managed
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_create_map() -> *mut CelValue {
    let map = CelValue::Object(HashMap::new());
    let boxed_map = Box::new(map);
    Box::into_raw(boxed_map)
}

/// Insert a key-value pair into a CelValue map.
///
/// # Parameters
/// - `map_ptr`: Pointer to a CelValue (must be an Object/HashMap variant)
/// - `key_ptr`: Pointer to a CelValue to use as the key (must be a String)
/// - `value_ptr`: Pointer to a CelValue to use as the value
///
/// # Panics
/// - If `map_ptr` is null
/// - If `key_ptr` is null
/// - If `value_ptr` is null
/// - If the map CelValue is not an Object
/// - If the key CelValue is not a String
///
/// # Safety
/// - All pointers must be valid CelValue pointers
/// - This function mutates the map in place
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_map_insert(
    map_ptr: *mut CelValue,
    key_ptr: *mut CelValue,
    value_ptr: *mut CelValue,
) {
    let log = crate::logging::get_logger();

    // Check for null pointers
    if map_ptr.is_null() {
        error!(log, "Map pointer is null";
            "function" => "cel_map_insert",
            "parameter" => "map_ptr");
        abort_with_error("no such overload");
    }
    if key_ptr.is_null() {
        error!(log, "Key pointer is null";
            "function" => "cel_map_insert",
            "parameter" => "key_ptr");
        abort_with_error("no such overload");
    }
    if value_ptr.is_null() {
        error!(log, "Value pointer is null";
            "function" => "cel_map_insert",
            "parameter" => "value_ptr");
        abort_with_error("no such overload");
    }

    // SAFETY: Caller guarantees all pointers are valid
    let map_value = unsafe { &mut *map_ptr };
    let key = unsafe { &*key_ptr };
    let value = unsafe { &*value_ptr };

    // Extract the key string
    let key_string = match key {
        CelValue::String(s) => s.clone(),
        _ => {
            error!(log, "Map key must be a String";
            "function" => "cel_map_insert",
            "expected_key_type" => "String",
            "actual_key_type" => format!("{:?}", key));
            abort_with_error("no such overload")
        }
    };

    // Insert the key-value pair into the map
    match map_value {
        CelValue::Object(hash_map) => {
            debug!(log, "Inserting into map"; 
                "key" => key_string.as_str(),
                "current_size" => hash_map.len());
            hash_map.insert(key_string, value.clone());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_empty_map() {
        let map_ptr = unsafe { cel_create_map() };
        let map_value = unsafe { &*map_ptr };

        match map_value {
            CelValue::Object(hash_map) => {
                assert!(hash_map.is_empty(), "New map should be empty");
            }
            _ => panic!("Expected Object, got {:?}", map_value),
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(map_ptr);
        }
    }

    #[test]
    fn test_map_insert_single_entry() {
        let map_ptr = unsafe { cel_create_map() };
        let key_ptr = Box::into_raw(Box::new(CelValue::String("name".to_string())));
        let value_ptr = Box::into_raw(Box::new(CelValue::String("Alice".to_string())));

        unsafe {
            cel_map_insert(map_ptr, key_ptr, value_ptr);
        }

        let map_value = unsafe { &*map_ptr };
        match map_value {
            CelValue::Object(hash_map) => {
                assert_eq!(hash_map.len(), 1, "Map should have 1 entry");
                assert_eq!(
                    hash_map.get("name"),
                    Some(&CelValue::String("Alice".to_string())),
                    "Map should contain the inserted key-value pair"
                );
            }
            _ => panic!("Expected Object, got {:?}", map_value),
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(map_ptr);
            let _ = Box::from_raw(key_ptr);
            let _ = Box::from_raw(value_ptr);
        }
    }

    #[test]
    fn test_map_insert_multiple_entries() {
        let map_ptr = unsafe { cel_create_map() };

        // Insert first entry
        let key1_ptr = Box::into_raw(Box::new(CelValue::String("name".to_string())));
        let value1_ptr = Box::into_raw(Box::new(CelValue::String("Alice".to_string())));
        unsafe {
            cel_map_insert(map_ptr, key1_ptr, value1_ptr);
        }

        // Insert second entry
        let key2_ptr = Box::into_raw(Box::new(CelValue::String("age".to_string())));
        let value2_ptr = Box::into_raw(Box::new(CelValue::Int(30)));
        unsafe {
            cel_map_insert(map_ptr, key2_ptr, value2_ptr);
        }

        // Verify both entries
        let map_value = unsafe { &*map_ptr };
        match map_value {
            CelValue::Object(hash_map) => {
                assert_eq!(hash_map.len(), 2, "Map should have 2 entries");
                assert_eq!(
                    hash_map.get("name"),
                    Some(&CelValue::String("Alice".to_string()))
                );
                assert_eq!(hash_map.get("age"), Some(&CelValue::Int(30)));
            }
            _ => panic!("Expected Object, got {:?}", map_value),
        }

        // Clean up
        unsafe {
            let _ = Box::from_raw(map_ptr);
            let _ = Box::from_raw(key1_ptr);
            let _ = Box::from_raw(value1_ptr);
            let _ = Box::from_raw(key2_ptr);
            let _ = Box::from_raw(value2_ptr);
        }
    }

    // Note: Cannot test panic cases with #[should_panic] for extern "C" functions
    // as they cause process aborts. Panic behavior is tested in integration tests.
}
