//! Membership testing operations for the `in` operator.
//!
//! Following CEL specification:
//! - A in list(A): checks if value exists in list (linear search)
//! - A in map(A, B): checks if key exists in map (key existence only)
//!
//! Per CEL spec:
//! - Time cost for lists: O(n×m) where n is list size, m is element size
//! - Time cost for maps: O(1) expected (implementation may vary)

use crate::helpers::cel_create_bool;
use crate::types::CelValue;

/// Check if an element exists in a container (list or map).
///
/// # Parameters
/// - `element_ptr`: Pointer to the value to search for
/// - `container_ptr`: Pointer to the container (Array or Object/map)
///
/// # Returns
/// - Pointer to CelValue::Bool(true) if element is found, false otherwise
///
/// # Panics
/// - If either pointer is null
/// - If types don't match CEL specification (no_matching_overload)
///
/// # Safety
/// - Both pointers must be valid CelValue pointers
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_in(
    element_ptr: *mut CelValue,
    container_ptr: *mut CelValue,
) -> *mut CelValue {
    if element_ptr.is_null() {
        panic!("cel_value_in: element_ptr is null");
    }
    if container_ptr.is_null() {
        panic!("cel_value_in: container_ptr is null");
    }

    let element = unsafe { &*element_ptr };
    let container = unsafe { &*container_ptr };

    match container {
        // List membership: A in list(A)
        CelValue::Array(arr) => {
            // Linear search through array for equality
            let found = arr.iter().any(|item| item == element);
            cel_create_bool(if found { 1 } else { 0 })
        }

        // Map key membership: A in map(A, B)
        // Only checks key existence, not values
        CelValue::Object(map) => match element {
            CelValue::String(key) => {
                // Maps in CEL must have string keys
                let found = map.contains_key(key);
                cel_create_bool(if found { 1 } else { 0 })
            }
            _ => {
                panic!("cel_value_in: maps require string keys, got {:?}", element);
            }
        },

        // Type mismatch - no matching overload
        _ => {
            panic!(
                "cel_value_in: no matching overload for {:?} in {:?}",
                element, container
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::collections::HashMap;

    // Helper function to test membership operations
    fn assert_membership(element: CelValue, container: CelValue, expected: bool) {
        let element_ptr = Box::into_raw(Box::new(element));
        let container_ptr = Box::into_raw(Box::new(container));

        unsafe {
            let result_ptr = cel_value_in(element_ptr, container_ptr);
            let result = match &*result_ptr {
                CelValue::Bool(b) => *b,
                _ => panic!("Expected Bool"),
            };
            assert_eq!(result, expected);

            // Cleanup
            let _ = Box::from_raw(element_ptr);
            let _ = Box::from_raw(container_ptr);
            let _ = Box::from_raw(result_ptr);
        }
    }

    // List membership tests
    #[rstest]
    #[case::int_in_list(
        CelValue::Int(2),
        CelValue::Array(vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3)]),
        true
    )]
    #[case::int_not_in_list(
        CelValue::Int(5),
        CelValue::Array(vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3)]),
        false
    )]
    #[case::string_in_list(
        CelValue::String("b".to_string()),
        CelValue::Array(vec![
            CelValue::String("a".to_string()),
            CelValue::String("b".to_string()),
            CelValue::String("c".to_string()),
        ]),
        true
    )]
    #[case::string_not_in_list(
        CelValue::String("d".to_string()),
        CelValue::Array(vec![
            CelValue::String("a".to_string()),
            CelValue::String("b".to_string()),
        ]),
        false
    )]
    #[case::bool_in_list(
        CelValue::Bool(true),
        CelValue::Array(vec![CelValue::Bool(false), CelValue::Bool(true)]),
        true
    )]
    #[case::empty_list(
        CelValue::Int(1),
        CelValue::Array(vec![]),
        false
    )]
    #[case::null_in_list(
        CelValue::Null,
        CelValue::Array(vec![CelValue::Null, CelValue::Int(1)]),
        true
    )]
    #[case::double_in_list(
        CelValue::Double(3.14),
        CelValue::Array(vec![CelValue::Double(1.0), CelValue::Double(2.0), CelValue::Double(3.14)]),
        true
    )]
    fn test_list_membership(
        #[case] element: CelValue,
        #[case] container: CelValue,
        #[case] expected: bool,
    ) {
        assert_membership(element, container, expected);
    }

    // Map key membership tests
    #[rstest]
    #[case::key_exists(
        CelValue::String("key1".to_string()),
        CelValue::Object({
            let mut map = HashMap::new();
            map.insert("key1".to_string(), CelValue::String("value1".to_string()));
            map.insert("key2".to_string(), CelValue::String("value2".to_string()));
            map
        }),
        true
    )]
    #[case::key_missing(
        CelValue::String("key3".to_string()),
        CelValue::Object({
            let mut map = HashMap::new();
            map.insert("key1".to_string(), CelValue::String("value1".to_string()));
            map
        }),
        false
    )]
    #[case::null_value_key_exists(
        CelValue::String("age".to_string()),
        CelValue::Object({
            let mut map = HashMap::new();
            map.insert("name".to_string(), CelValue::String("Alice".to_string()));
            map.insert("age".to_string(), CelValue::Null);
            map
        }),
        true  // Key exists even though value is null
    )]
    #[case::empty_map(
        CelValue::String("key".to_string()),
        CelValue::Object(HashMap::new()),
        false
    )]
    fn test_map_membership(
        #[case] element: CelValue,
        #[case] container: CelValue,
        #[case] expected: bool,
    ) {
        assert_membership(element, container, expected);
    }

    // Note: Cannot test panic cases with #[should_panic] for extern "C" functions
    // as they cause process aborts. Panic behavior is tested in integration tests.
}
