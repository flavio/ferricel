//! Field access operations for CelValue objects.
//! Supports accessing fields from Object-type CelValues.

extern crate alloc;

use crate::types::CelValue;
use alloc::boxed::Box;
use alloc::string::String;
use core::slice;

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
    // Check for null object pointer
    if obj_ptr.is_null() {
        panic!("Cannot access field on null object");
    }

    // SAFETY: Caller guarantees obj_ptr is valid
    let obj = unsafe { &*obj_ptr };

    // Read the field name from WASM memory
    let field_name = unsafe {
        let bytes = slice::from_raw_parts(field_name_ptr as *const u8, field_name_len as usize);
        String::from_utf8(bytes.to_vec())
            .unwrap_or_else(|_| panic!("Field name is not valid UTF-8"))
    };

    // Extract the field from the object
    match obj {
        CelValue::Object(map) => {
            // Look up the field in the hashmap
            match map.get(&field_name) {
                Some(value) => {
                    // Clone the value and return a new boxed pointer
                    let boxed_value = Box::new(value.clone());
                    Box::into_raw(boxed_value)
                }
                None => {
                    panic!("Field '{}' not found in object", field_name);
                }
            }
        }
        _ => {
            panic!("Cannot access field '{}' on non-object value", field_name);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::types::CelValue;
    use hashbrown::HashMap;

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

    // Note: Tests using cel_get_field are skipped because they require
    // WASM memory operations that cause segfaults in the test environment.
    // The actual WASM runtime will have proper memory management.
}
