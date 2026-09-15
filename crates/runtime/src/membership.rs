//! Membership testing operations for the `in` operator.
//!
//! Following CEL specification:
//! - A in list(A): checks if value exists in list (linear search)
//! - A in map(A, B): checks if key exists in map (key existence only)
//!
//! Per CEL spec:
//! - Time cost for lists: O(n×m) where n is list size, m is element size
//! - Time cost for maps: O(1) expected (implementation may vary)
//!
//! `value_in` returns `CelValue::Error` for a non-collection container, an
//! invalid map-key element, or an error operand, instead of aborting: this
//! is what lets `1 in object.missing` evaluate to the "no such key" error
//! instead of a trap.

use slog::debug;

use crate::{
    error::{abort_with_error, no_such_overload},
    helpers::cel_equals,
    memory::read_ptr,
    types::CelValue,
};

/// Check if an element exists in a container (list or map).
///
/// # Parameters
/// - `element_ptr`: Pointer to the value to search for
/// - `container_ptr`: Pointer to the container (Array or Object/map)
///
/// # Returns
/// - Pointer to CelValue::Bool(true) if element is found, false otherwise
/// - Pointer to CelValue::Error for a non-collection container, an invalid
///   map-key element, or a propagated error operand
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue instances
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_in(
    element_ptr: *mut CelValue,
    container_ptr: *mut CelValue,
) -> *mut CelValue {
    // A null pointer here is a compiler bug: the compiler always evaluates
    // both operands to a CelValue before calling this function.
    if element_ptr.is_null() {
        abort_with_error("cel_value_in: null element pointer, this is a compiler bug");
    }
    if container_ptr.is_null() {
        abort_with_error("cel_value_in: null container pointer, this is a compiler bug");
    }

    let element = read_ptr(element_ptr);
    let container = read_ptr(container_ptr);

    Box::into_raw(Box::new(value_in(element, container)))
}

fn value_in(element: CelValue, container: CelValue) -> CelValue {
    let log = crate::logging::get_logger();

    match (element, container) {
        (CelValue::Error(e), _) => CelValue::Error(e),
        (_, CelValue::Error(e)) => CelValue::Error(e),

        // List membership: A in list(A)
        (element, CelValue::Array(arr)) => {
            debug!(log, "Checking list membership"; "list_size" => arr.len());
            // Linear search through array using CEL equality (supports cross-type numeric equality)
            let found = arr.iter().any(|item| cel_equals(item, &element));
            debug!(log, "List membership check complete"; "found" => found);
            CelValue::Bool(found)
        }

        // Map key membership: A in map(A, B)
        // Only checks key existence, not values
        // Maps can have bool, int, uint, or string keys per CEL spec
        (element, CelValue::Object(map)) => {
            use crate::types::CelMapKey;
            match CelMapKey::from_cel_value(&element) {
                Some(key) => {
                    debug!(log, "Checking map key membership";
                        "key" => key.to_string_key(),
                        "map_size" => map.len());
                    let found = map.contains_key(&key);
                    debug!(log, "Map membership check complete"; "found" => found);
                    CelValue::Bool(found)
                }
                None => {
                    debug!(log, "Maps require bool, int, uint, or string keys for membership test";
                        "actual_key_type" => format!("{:?}", element));
                    CelValue::Error(crate::error::CelError::new(
                        "no such overload: invalid map key type",
                    ))
                }
            }
        }

        // Type mismatch - no matching overload
        (element, container) => {
            debug!(log, "No matching overload for membership test";
                "element_type" => format!("{:?}", element),
                "container_type" => format!("{:?}", container));
            CelValue::Error(no_such_overload(&container))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rstest::rstest;

    use super::*;
    use crate::error::CelError;

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

            // element_ptr and container_ptr are consumed by cel_value_in; only free the result
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
        CelValue::Double(3.15),
        CelValue::Array(vec![CelValue::Double(1.0), CelValue::Double(2.0), CelValue::Double(3.15)]),
        true
    )]
    #[case::bytes_in_list(
        CelValue::Bytes(vec![0x68, 0x69]),  // "hi"
        CelValue::Array(vec![
            CelValue::Bytes(vec![0x61, 0x62]),  // "ab"
            CelValue::Bytes(vec![0x68, 0x69]),  // "hi"
            CelValue::Bytes(vec![0x78, 0x79]),  // "xy"
        ]),
        true
    )]
    #[case::bytes_not_in_list(
        CelValue::Bytes(vec![0xFF, 0xFE]),
        CelValue::Array(vec![
            CelValue::Bytes(vec![0x00, 0x01]),
            CelValue::Bytes(vec![0x02, 0x03]),
        ]),
        false
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
            use crate::types::CelMapKey;
            let mut map = HashMap::new();
            map.insert(CelMapKey::String("key1".to_string()), CelValue::String("value1".to_string()));
            map.insert(CelMapKey::String("key2".to_string()), CelValue::String("value2".to_string()));
            map
        }),
        true
    )]
    #[case::key_missing(
        CelValue::String("key3".to_string()),
        CelValue::Object({
            use crate::types::CelMapKey;
            let mut map = HashMap::new();
            map.insert(CelMapKey::String("key1".to_string()), CelValue::String("value1".to_string()));
            map
        }),
        false
    )]
    #[case::null_value_key_exists(
        CelValue::String("age".to_string()),
        CelValue::Object({
            use crate::types::CelMapKey;
            let mut map = HashMap::new();
            map.insert(CelMapKey::String("name".to_string()), CelValue::String("Alice".to_string()));
            map.insert(CelMapKey::String("age".to_string()), CelValue::Null);
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

    /// A non-collection container, an invalid map-key element, and a
    /// propagated error operand are all error values, not traps.
    #[rstest]
    #[case::wrong_container_type(CelValue::Int(1), CelValue::Int(2), "no such overload")]
    #[case::invalid_map_key(
        CelValue::Double(1.5),
        CelValue::Object(HashMap::new()),
        "invalid map key type"
    )]
    #[case::element_is_error(
        CelValue::Error(CelError::new("no such key: 'x'")),
        CelValue::Array(vec![]),
        "no such key: 'x'"
    )]
    #[case::container_is_error(
        CelValue::Int(1),
        CelValue::Error(CelError::new("no such key: 'y'")),
        "no such key: 'y'"
    )]
    fn test_value_in_rejects_bad_input(
        #[case] element: CelValue,
        #[case] container: CelValue,
        #[case] expected_substring: &str,
    ) {
        match value_in(element, container) {
            CelValue::Error(err) => assert!(
                err.message.contains(expected_substring),
                "expected {expected_substring:?} in {:?}",
                err.message
            ),
            other => panic!("expected an error value, got {other:?}"),
        }
    }

    // Note: Cannot test panic cases with #[should_panic] for extern "C" functions
    // as they cause process aborts. Panic behavior is tested in integration tests.
}
