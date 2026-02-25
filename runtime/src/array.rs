//! Array operations for CelValue objects.
//! Supports creating arrays, accessing elements, array concatenation, and getting array length.

use crate::cel_panic;
use crate::logging::macros::cel_debug;
use crate::types::CelValue;

/// Internal helper: Concatenate two arrays.
///
/// # Arguments
/// - `a`: First array slice
/// - `b`: Second array slice
///
/// # Returns
/// A new Vec containing all elements from `a` followed by all elements from `b`
pub(crate) fn cel_array_concat(a: &[CelValue], b: &[CelValue]) -> Vec<CelValue> {
    let mut result = Vec::with_capacity(a.len() + b.len());
    result.extend_from_slice(a);
    result.extend_from_slice(b);
    result
}

/// Get the length of a CelValue array.
///
/// # Parameters
/// - `array_ptr`: Pointer to a CelValue (must be an Array variant)
///
/// # Returns
/// - The length of the array as an i32
///
/// # Panics
/// - If `array_ptr` is null
/// - If the CelValue is not an Array
///
/// # Safety
/// - `array_ptr` must be a valid pointer to a CelValue
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_array_len(array_ptr: *mut CelValue) -> i32 {
    let log = crate::logging::get_logger();

    // Check for null array pointer
    if array_ptr.is_null() {
        cel_panic!(log, "Cannot get length of null array";
            "function" => "cel_array_len");
    }

    // SAFETY: Caller guarantees array_ptr is valid
    let array_value = unsafe { &*array_ptr };

    // Extract the length from the array
    match array_value {
        CelValue::Array(vec) => {
            cel_debug!(log, "Getting array length"; "length" => vec.len());
            vec.len() as i32
        }
        _ => cel_panic!(log, "Type mismatch in array operation";
            "function" => "cel_array_len",
            "expected" => "Array",
            "actual" => format!("{:?}", array_value)),
    }
}

/// Get an element from a CelValue array at a specific index.
///
/// # Parameters
/// - `array_ptr`: Pointer to a CelValue (must be an Array variant)
/// - `index`: The index to access (0-based)
///
/// # Returns
/// - Pointer to a new boxed CelValue containing the element at the given index
///
/// # Panics
/// - If `array_ptr` is null
/// - If the CelValue is not an Array
/// - If the index is out of bounds
///
/// # Safety
/// - `array_ptr` must be a valid pointer to a CelValue
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_array_get(array_ptr: *mut CelValue, index: i32) -> *mut CelValue {
    let log = crate::logging::get_logger();

    // Check for null array pointer
    if array_ptr.is_null() {
        cel_panic!(log, "Cannot get element from null array";
            "function" => "cel_array_get");
    }

    // SAFETY: Caller guarantees array_ptr is valid
    let array_value = unsafe { &*array_ptr };

    // Extract the element from the array
    match array_value {
        CelValue::Array(vec) => {
            cel_debug!(log, "Accessing array element"; "index" => index, "length" => vec.len());
            let idx = index as usize;
            if idx >= vec.len() {
                cel_panic!(log, "Array index out of bounds";
                    "function" => "cel_array_get",
                    "index" => index,
                    "length" => vec.len());
            }
            // Clone the element and return a new boxed pointer
            let boxed_value = Box::new(vec[idx].clone());
            Box::into_raw(boxed_value)
        }
        _ => cel_panic!(log, "Type mismatch in array operation";
            "function" => "cel_array_get",
            "expected" => "Array",
            "actual" => format!("{:?}", array_value)),
    }
}

/// Create a new empty CelValue array.
///
/// # Returns
/// - Pointer to a new boxed CelValue containing an empty Array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_create_array() -> *mut CelValue {
    let array = CelValue::Array(Vec::new());
    let boxed_array = Box::new(array);
    Box::into_raw(boxed_array)
}

/// Push an element to a CelValue array.
///
/// # Parameters
/// - `array_ptr`: Pointer to a CelValue (must be an Array variant)
/// - `element_ptr`: Pointer to a CelValue to push to the array
///
/// # Panics
/// - If `array_ptr` is null
/// - If `element_ptr` is null
/// - If the CelValue is not an Array
///
/// # Safety
/// - `array_ptr` must be a valid pointer to a CelValue
/// - `element_ptr` must be a valid pointer to a CelValue
/// - This function mutates the array in place
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_array_push(array_ptr: *mut CelValue, element_ptr: *mut CelValue) {
    let log = crate::logging::get_logger();

    // Check for null pointers
    if array_ptr.is_null() {
        cel_panic!(log, "Cannot push to null array";
            "function" => "cel_array_push");
    }
    if element_ptr.is_null() {
        cel_panic!(log, "Cannot push null element to array";
            "function" => "cel_array_push");
    }

    // SAFETY: Caller guarantees both pointers are valid
    let array_value = unsafe { &mut *array_ptr };
    let element = unsafe { &*element_ptr };

    // Push the element to the array
    match array_value {
        CelValue::Array(vec) => {
            cel_debug!(log, "Pushing element to array"; "current_length" => vec.len());
            vec.push(element.clone());
        }
        _ => cel_panic!(log, "Type mismatch in array operation";
            "function" => "cel_array_push",
            "expected" => "Array",
            "actual" => format!("{:?}", array_value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_create_array_logic() {
        let array = CelValue::Array(Vec::new());
        match array {
            CelValue::Array(ref vec) => assert_eq!(vec.len(), 0),
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_array_len_logic() {
        // Test empty array
        let empty_array = CelValue::Array(Vec::new());
        match empty_array {
            CelValue::Array(ref vec) => assert_eq!(vec.len(), 0),
            _ => panic!("Expected Array"),
        }

        // Test array with elements
        let array = CelValue::Array(vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3)]);
        match array {
            CelValue::Array(ref vec) => assert_eq!(vec.len(), 3),
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_array_get_logic() {
        let array = CelValue::Array(vec![
            CelValue::Int(10),
            CelValue::Int(20),
            CelValue::Int(30),
        ]);

        match array {
            CelValue::Array(ref vec) => {
                assert_eq!(vec[0], CelValue::Int(10));
                assert_eq!(vec[1], CelValue::Int(20));
                assert_eq!(vec[2], CelValue::Int(30));
            }
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_array_push_logic() {
        let mut vec = Vec::new();
        vec.push(CelValue::Int(42));
        vec.push(CelValue::Bool(true));
        vec.push(CelValue::Int(3));

        assert_eq!(vec.len(), 3);
        assert_eq!(vec[0], CelValue::Int(42));
        assert_eq!(vec[1], CelValue::Bool(true));
        assert_eq!(vec[2], CelValue::Int(3));
    }

    #[test]
    fn test_create_array_ffi() {
        let array_ptr = unsafe { cel_create_array() };
        assert!(!array_ptr.is_null());

        let array = unsafe { &*array_ptr };
        match array {
            CelValue::Array(vec) => assert_eq!(vec.len(), 0),
            _ => panic!("Expected Array"),
        }

        // Cleanup
        unsafe { drop(Box::from_raw(array_ptr)) };
    }

    #[test]
    fn test_array_get_ffi() {
        let array = Box::new(CelValue::Array(vec![
            CelValue::Int(10),
            CelValue::Int(20),
            CelValue::Int(30),
        ]));
        let array_ptr = Box::into_raw(array);

        let element_ptr = unsafe { cel_array_get(array_ptr, 1) };
        assert!(!element_ptr.is_null());

        let element = unsafe { &*element_ptr };
        match element {
            CelValue::Int(val) => assert_eq!(*val, 20),
            _ => panic!("Expected Int"),
        }

        // Cleanup
        unsafe {
            drop(Box::from_raw(element_ptr));
            drop(Box::from_raw(array_ptr));
        }
    }

    #[test]
    fn test_array_push_ffi() {
        let array = Box::new(CelValue::Array(Vec::new()));
        let array_ptr = Box::into_raw(array);

        let elem1 = Box::new(CelValue::Int(1));
        let elem1_ptr = Box::into_raw(elem1);
        let elem2 = Box::new(CelValue::Bool(true));
        let elem2_ptr = Box::into_raw(elem2);

        unsafe {
            cel_array_push(array_ptr, elem1_ptr);
            cel_array_push(array_ptr, elem2_ptr);
        }

        let array = unsafe { &*array_ptr };
        match array {
            CelValue::Array(vec) => {
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], CelValue::Int(1));
                assert_eq!(vec[1], CelValue::Bool(true));
            }
            _ => panic!("Expected Array"),
        }

        // Cleanup
        unsafe {
            drop(Box::from_raw(elem1_ptr));
            drop(Box::from_raw(elem2_ptr));
            drop(Box::from_raw(array_ptr));
        }
    }

    #[rstest]
    #[case::both_empty(&[], &[], &[])]
    #[case::first_empty(&[], &[CelValue::Int(1)], &[CelValue::Int(1)])]
    #[case::second_empty(&[CelValue::Int(1)], &[], &[CelValue::Int(1)])]
    #[case::both_single(&[CelValue::Int(1)], &[CelValue::Int(2)], &[CelValue::Int(1), CelValue::Int(2)])]
    #[case::multiple(&[CelValue::Int(1), CelValue::Int(2)], &[CelValue::Int(3), CelValue::Int(4)], &[CelValue::Int(1), CelValue::Int(2), CelValue::Int(3), CelValue::Int(4)])]
    #[case::mixed_types(&[CelValue::Int(1), CelValue::Bool(true)], &[CelValue::Int(2)], &[CelValue::Int(1), CelValue::Bool(true), CelValue::Int(2)])]
    fn test_array_concat(
        #[case] a: &[CelValue],
        #[case] b: &[CelValue],
        #[case] expected: &[CelValue],
    ) {
        let result = cel_array_concat(a, b);
        assert_eq!(result.as_slice(), expected);
    }
}
