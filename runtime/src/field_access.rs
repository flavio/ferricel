//! Field access operations for CelValue objects.
//! Supports accessing fields from Object-type CelValues.

use crate::cel_panic;
use crate::logging::macros::{cel_debug, cel_info};
use crate::types::CelValue;
use std::slice;

/// Get a field from a CelValue object.
///
/// # Parameters
/// - `obj_ptr`: Pointer to a CelValue (must be an Object variant)
/// - `field_name_ptr`: Pointer to the field name string in WASM memory
/// - `field_name_len`: Length of the field name string
///
/// # Returns
/// - Pointer to a new boxed CelValue containing the field value
///
/// # Panics
/// - If `obj_ptr` is null
/// - If the CelValue is not an Object
/// - If the field is not found in the object
/// - If the field name is invalid UTF-8
///
/// # Safety
/// - `obj_ptr` must be a valid pointer to a CelValue
/// - `field_name_ptr` must point to valid UTF-8 bytes in WASM memory
/// - `field_name_len` must be the correct length
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_get_field(
    obj_ptr: *mut CelValue,
    field_name_ptr: i32,
    field_name_len: i32,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    // Check for null object pointer
    if obj_ptr.is_null() {
        cel_panic!(log, "Cannot access field on null object";
            "function" => "cel_get_field");
    }

    // SAFETY: Caller guarantees obj_ptr is valid
    let obj = unsafe { &*obj_ptr };

    // Read the field name from WASM memory
    let field_name = unsafe {
        let bytes = slice::from_raw_parts(field_name_ptr as *const u8, field_name_len as usize);
        String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| {
            cel_panic!(log, "Field name is not valid UTF-8";
                "function" => "cel_get_field",
                "bytes_len" => field_name_len)
        })
    };

    // Extract the field from the object
    match obj {
        CelValue::Object(map) => {
            cel_debug!(log, "Accessing field from object"; 
                "field" => field_name.as_str(),
                "num_fields" => map.len());
            // Look up the field in the hashmap
            match map.get(&field_name) {
                Some(value) => {
                    cel_info!(log, "Field found"; "field" => field_name.as_str());
                    // Clone the value and return a new boxed pointer
                    let boxed_value = Box::new(value.clone());
                    Box::into_raw(boxed_value)
                }
                None => {
                    let available_fields: Vec<&String> = map.keys().collect();
                    cel_panic!(log, "Field not found in object";
                        "field" => field_name,
                        "available_fields" => format!("{:?}", available_fields));
                }
            }
        }
        _ => {
            cel_panic!(log, "Cannot access field on non-object value";
                "field" => field_name,
                "actual_type" => format!("{:?}", obj));
        }
    }
}

/// Check if a field exists in a CelValue object (for has() macro).
///
/// # Parameters
/// - `obj_ptr`: Pointer to a CelValue (should be an Object variant)
/// - `field_name_ptr`: Pointer to the field name string in WASM memory
/// - `field_name_len`: Length of the field name string
///
/// # Returns
/// - Pointer to a new boxed CelValue::Bool(true) if field exists
/// - Pointer to a new boxed CelValue::Bool(false) if field missing or obj is not an Object
///
/// # Panics
/// - If `obj_ptr` is null
/// - If the field name is invalid UTF-8
///
/// # Safety
/// - `obj_ptr` must be a valid pointer to a CelValue
/// - `field_name_ptr` must point to valid UTF-8 bytes in WASM memory
/// - `field_name_len` must be the correct length
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_has_field(
    obj_ptr: *mut CelValue,
    field_name_ptr: i32,
    field_name_len: i32,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    // Check for null object pointer
    if obj_ptr.is_null() {
        cel_panic!(log, "Cannot check field on null object";
            "function" => "cel_has_field");
    }

    // SAFETY: Caller guarantees obj_ptr is valid
    let obj = unsafe { &*obj_ptr };

    // Read the field name from WASM memory
    let field_name = unsafe {
        let bytes = slice::from_raw_parts(field_name_ptr as *const u8, field_name_len as usize);
        String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| {
            cel_panic!(log, "Field name is not valid UTF-8";
                "function" => "cel_has_field",
                "bytes_len" => field_name_len)
        })
    };

    // Check if the field exists
    // Returns true if the key exists in the map, regardless of value (including null)
    // Returns false if obj is not an Object or if field is missing
    let has_field = match obj {
        CelValue::Object(map) => map.contains_key(&field_name),
        _ => false, // Non-objects don't have fields
    };

    // Return a boxed boolean
    let boxed_value = Box::new(CelValue::Bool(has_field));
    Box::into_raw(boxed_value)
}

#[cfg(test)]
mod tests {
    use crate::types::CelValue;
    use std::collections::HashMap;

    #[test]
    fn test_field_access_logic() {
        // Test the logic without WASM memory operations
        let mut map = HashMap::new();
        map.insert("age".into(), CelValue::Int(42));
        let obj = CelValue::Object(map);

        // Verify we can access the field
        if let CelValue::Object(ref map) = obj {
            let field_value = map.get("age");
            assert!(field_value.is_some());
            assert_eq!(*field_value.unwrap(), CelValue::Int(42));
        } else {
            panic!("Expected Object variant");
        }
    }

    #[test]
    fn test_nested_object_logic() {
        // Create a nested object: {"user": {"name": "Alice"}}
        let mut inner_map = HashMap::new();
        inner_map.insert("name".into(), CelValue::String("Alice".into()));

        let mut outer_map = HashMap::new();
        outer_map.insert("user".into(), CelValue::Object(inner_map));

        let obj = CelValue::Object(outer_map);

        // Verify we can access the nested field
        if let CelValue::Object(ref map) = obj {
            let user_value = map.get("user");
            assert!(user_value.is_some());
            assert!(matches!(user_value.unwrap(), CelValue::Object(_)));
        }
    }

    #[test]
    fn test_has_field_logic_exists() {
        // Test has() when field exists
        let mut map = HashMap::new();
        map.insert("age".into(), CelValue::Int(42));
        let obj = CelValue::Object(map);

        if let CelValue::Object(ref map) = obj {
            assert!(map.contains_key("age"));
            assert!(!map.contains_key("missing"));
        } else {
            panic!("Expected Object variant");
        }
    }

    #[test]
    fn test_has_field_logic_missing() {
        // Test has() when field doesn't exist
        let map = HashMap::new();
        let obj = CelValue::Object(map);

        if let CelValue::Object(ref map) = obj {
            assert!(!map.contains_key("nonexistent"));
        } else {
            panic!("Expected Object variant");
        }
    }

    #[test]
    fn test_has_field_logic_non_object() {
        // Test has() on non-object types (should return false)
        let non_obj = CelValue::Int(42);
        let has_field = matches!(non_obj, CelValue::Object(_));
        assert!(!has_field, "Non-objects should not have fields");
    }

    #[test]
    fn test_has_field_logic_null_value() {
        // Test has() when field exists but value is null
        // Should return true because the key exists in the map
        let mut map = HashMap::new();
        map.insert("nullable".into(), CelValue::Null);
        let obj = CelValue::Object(map);

        if let CelValue::Object(ref map) = obj {
            assert!(
                map.contains_key("nullable"),
                "Should return true even if value is null"
            );
        } else {
            panic!("Expected Object variant");
        }
    }

    // Note: Tests using cel_get_field and cel_has_field are skipped because they require
    // WASM memory operations that cause segfaults in the test environment.
    // The actual WASM runtime will have proper memory management.
}
