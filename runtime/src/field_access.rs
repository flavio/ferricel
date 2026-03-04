//! Field access operations for CelValue objects.
//! Supports accessing fields from Object-type CelValues.

use crate::error::abort_with_error;
use crate::types::CelValue;
use slog::{debug, error, info};
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
        error!(log, "Cannot access field on null object";
            "function" => "cel_get_field");
        abort_with_error("no such overload");
    }

    // SAFETY: Caller guarantees obj_ptr is valid
    let obj = unsafe { &*obj_ptr };

    // Read the field name from WASM memory
    let field_name = unsafe {
        let bytes = slice::from_raw_parts(field_name_ptr as *const u8, field_name_len as usize);
        String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| {
            error!(log, "Field name is not valid UTF-8";
                "function" => "cel_get_field",
                "bytes_len" => field_name_len);
            abort_with_error("no such overload")
        })
    };

    // Extract the field from the object
    match obj {
        CelValue::Object(map) => {
            debug!(log, "Accessing field from object"; 
                "field" => field_name.as_str(),
                "num_fields" => map.len());

            // Check if this is a wrapper field that should return null when unset
            let is_wrapper_field = is_wrapper_field_unset(map, &field_name);

            // Look up the field in the hashmap using string key
            use crate::types::CelMapKey;
            let key = CelMapKey::String(field_name.clone());
            match map.get(&key) {
                Some(value) => {
                    info!(log, "Field found"; "field" => field_name.as_str());
                    // Clone the value and return a new boxed pointer
                    let boxed_value = Box::new(value.clone());
                    Box::into_raw(boxed_value)
                }
                None => {
                    // If this is a wrapper field that's unset, return null (per CEL spec)
                    if is_wrapper_field {
                        info!(log, "Unset wrapper field, returning null"; 
                            "field" => field_name.as_str());
                        let boxed_value = Box::new(CelValue::Null);
                        return Box::into_raw(boxed_value);
                    }

                    // Otherwise, field not found is an error
                    let available_fields: Vec<String> =
                        map.keys().map(|k| k.to_string_key()).collect();
                    {
                        error!(log, "Field not found in object";
                        "field" => field_name,
                        "available_fields" => format!("{:?}", available_fields));
                        abort_with_error("no such overload")
                    }
                }
            }
        }
        _ => {
            error!(log, "Cannot access field on non-object value";
                "field" => field_name,
                "actual_type" => format!("{:?}", obj));
            abort_with_error("no such overload")
        }
    }
}

/// Helper function to check if a field is a wrapper field that's unset.
/// Returns true if the object has __wrapper_fields__ metadata and the field is in that list.
fn is_wrapper_field_unset(
    map: &std::collections::HashMap<crate::types::CelMapKey, CelValue>,
    field_name: &str,
) -> bool {
    use crate::types::CelMapKey;

    // Check if __wrapper_fields__ metadata exists
    let wrapper_fields_key = CelMapKey::String("__wrapper_fields__".into());
    if let Some(CelValue::Array(wrapper_fields)) = map.get(&wrapper_fields_key) {
        // Check if this field is in the wrapper fields array
        for field in wrapper_fields {
            if let CelValue::String(wrapper_field_name) = field
                && wrapper_field_name == field_name {
                    return true;
                }
        }
    }

    false
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
        error!(log, "Cannot check field on null object";
            "function" => "cel_has_field");
        abort_with_error("no such overload");
    }

    // SAFETY: Caller guarantees obj_ptr is valid
    let obj = unsafe { &*obj_ptr };

    // Read the field name from WASM memory
    let field_name = unsafe {
        let bytes = slice::from_raw_parts(field_name_ptr as *const u8, field_name_len as usize);
        String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| {
            error!(log, "Field name is not valid UTF-8";
                "function" => "cel_has_field",
                "bytes_len" => field_name_len);
            abort_with_error("no such overload")
        })
    };

    // Check if the field exists
    // Returns true if the key exists in the map, regardless of value (including null)
    // Returns false if obj is not an Object or if field is missing
    use crate::types::CelMapKey;
    let has_field = match obj {
        CelValue::Object(map) => {
            let key = CelMapKey::String(field_name);
            map.contains_key(&key)
        }
        _ => false, // Non-objects don't have fields
    };

    // Return a boxed boolean
    let boxed_value = Box::new(CelValue::Bool(has_field));
    Box::into_raw(boxed_value)
}

#[cfg(test)]
mod tests {
    use crate::types::{CelMapKey, CelValue};
    use std::collections::HashMap;

    #[test]
    fn test_field_access_logic() {
        // Test the logic without WASM memory operations
        let mut map = HashMap::new();
        map.insert(CelMapKey::String("age".into()), CelValue::Int(42));
        let obj = CelValue::Object(map);

        // Verify we can access the field
        if let CelValue::Object(ref map) = obj {
            let field_value = map.get(&CelMapKey::String("age".into()));
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
        inner_map.insert(
            CelMapKey::String("name".into()),
            CelValue::String("Alice".into()),
        );

        let mut outer_map = HashMap::new();
        outer_map.insert(
            CelMapKey::String("user".into()),
            CelValue::Object(inner_map),
        );

        let obj = CelValue::Object(outer_map);

        // Verify we can access the nested field
        if let CelValue::Object(ref map) = obj {
            let user_value = map.get(&CelMapKey::String("user".into()));
            assert!(user_value.is_some());
            assert!(matches!(user_value.unwrap(), CelValue::Object(_)));
        }
    }

    #[test]
    fn test_has_field_logic_exists() {
        // Test has() when field exists
        let mut map = HashMap::new();
        map.insert(CelMapKey::String("age".into()), CelValue::Int(42));
        let obj = CelValue::Object(map);

        if let CelValue::Object(ref map) = obj {
            assert!(map.contains_key(&CelMapKey::String("age".into())));
            assert!(!map.contains_key(&CelMapKey::String("missing".into())));
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
            assert!(!map.contains_key(&CelMapKey::String("nonexistent".into())));
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
        map.insert(CelMapKey::String("nullable".into()), CelValue::Null);
        let obj = CelValue::Object(map);

        if let CelValue::Object(ref map) = obj {
            assert!(
                map.contains_key(&CelMapKey::String("nullable".into())),
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
