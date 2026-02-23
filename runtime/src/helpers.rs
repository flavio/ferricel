//! Helper functions for creating and extracting CelValue pointers.
//! These are used internally by other runtime functions and exported for compiler use.
//! Also includes polymorphic operators that dispatch to type-specific implementations.

use crate::types::CelValue;
use crate::{arithmetic, array, string};

/// Creates a CelValue::Int on the heap and returns a pointer to it.
/// The caller is responsible for freeing the memory using cel_free_value.
#[unsafe(no_mangle)]
pub extern "C" fn cel_create_int(value: i64) -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::Int(value)))
}

/// Creates a CelValue::Bool on the heap and returns a pointer to it.
/// Input: i64 where 0 = false, non-zero = true
/// The caller is responsible for freeing the memory using cel_free_value.
#[unsafe(no_mangle)]
pub extern "C" fn cel_create_bool(value: i64) -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::Bool(value != 0)))
}

/// Internal helper: Extracts i64 from CelValue or panics with type error.
/// This is not exported - it's used by arithmetic and comparison operations.
pub(crate) fn extract_int(ptr: *mut CelValue) -> i64 {
    unsafe {
        if ptr.is_null() {
            panic!("Null pointer passed to extract_int");
        }
        match &*ptr {
            CelValue::Int(i) => *i,
            other => panic!("Type error: expected Int, got {:?}", other),
        }
    }
}

/// Internal helper: Extracts bool from CelValue or panics with type error.
/// This is not exported - it's used by logical operations.
pub(crate) fn extract_bool(ptr: *mut CelValue) -> bool {
    unsafe {
        if ptr.is_null() {
            panic!("Null pointer passed to extract_bool");
        }
        match &*ptr {
            CelValue::Bool(b) => *b,
            other => panic!("Type error: expected Bool, got {:?}", other),
        }
    }
}

/// Polymorphic addition operator for CelValue objects.
/// Dispatches to type-specific implementations:
/// - Int + Int = Int (arithmetic addition)
/// - String + String = String (concatenation)
/// - Array + Array = Array (concatenation)
///
/// # Safety
/// - Both pointers must be valid, non-null CelValue pointers
///
/// # Arguments
/// - `a_ptr`: Pointer to the first operand
/// - `b_ptr`: Pointer to the second operand
///
/// # Returns
/// Pointer to a new heap-allocated CelValue containing the result
///
/// # Panics
/// - If either pointer is null
/// - If the operand types don't match
/// - If the operation is not supported for the given types
/// - On integer overflow (for Int addition)
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_add(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    unsafe {
        if a_ptr.is_null() || b_ptr.is_null() {
            panic!("Cannot add null values");
        }

        let a_val = &*a_ptr;
        let b_val = &*b_ptr;

        match (a_val, b_val) {
            (CelValue::Int(a), CelValue::Int(b)) => {
                let result = arithmetic::cel_int_add(*a, *b);
                cel_create_int(result)
            }
            (CelValue::String(a_str), CelValue::String(b_str)) => {
                let result = string::cel_string_concat(a_str, b_str);
                Box::into_raw(Box::new(CelValue::String(result)))
            }
            (CelValue::Array(a_vec), CelValue::Array(b_vec)) => {
                let result = array::cel_array_concat(a_vec, b_vec);
                Box::into_raw(Box::new(CelValue::Array(result)))
            }
            _ => panic!(
                "Cannot add {:?} and {:?}: unsupported types for addition",
                a_val, b_val
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deserialization::cel_free_value;
    use rstest::rstest;

    #[test]
    fn test_create_int() {
        let ptr = cel_create_int(42);
        unsafe {
            assert_eq!(*ptr, CelValue::Int(42));
            // Clean up
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_bool_true() {
        let ptr = cel_create_bool(1);
        unsafe {
            assert_eq!(*ptr, CelValue::Bool(true));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_bool_false() {
        let ptr = cel_create_bool(0);
        unsafe {
            assert_eq!(*ptr, CelValue::Bool(false));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_bool_nonzero() {
        let ptr = cel_create_bool(42);
        unsafe {
            assert_eq!(*ptr, CelValue::Bool(true));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_extract_int() {
        let ptr = cel_create_int(123);
        let value = extract_int(ptr);
        assert_eq!(value, 123);
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_extract_bool_true() {
        let ptr = cel_create_bool(1);
        let value = extract_bool(ptr);
        assert_eq!(value, true);
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_extract_bool_false() {
        let ptr = cel_create_bool(0);
        let value = extract_bool(ptr);
        assert_eq!(value, false);
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }

    // Integration tests for cel_value_add dispatcher

    #[rstest]
    #[case::int_add(CelValue::Int(2), CelValue::Int(3), CelValue::Int(5))]
    #[case::int_negative(CelValue::Int(-5), CelValue::Int(3), CelValue::Int(-2))]
    #[case::string_basic(
        CelValue::String("hello".to_string()),
        CelValue::String(" world".to_string()),
        CelValue::String("hello world".to_string())
    )]
    #[case::string_empty(
        CelValue::String("".to_string()),
        CelValue::String("test".to_string()),
        CelValue::String("test".to_string())
    )]
    #[case::string_unicode(
        CelValue::String("Hello ".to_string()),
        CelValue::String("世界".to_string()),
        CelValue::String("Hello 世界".to_string())
    )]
    #[case::string_emoji(
        CelValue::String("Hello ".to_string()),
        CelValue::String("👋🌍".to_string()),
        CelValue::String("Hello 👋🌍".to_string())
    )]
    #[case::array_both_empty(
        CelValue::Array(vec![]),
        CelValue::Array(vec![]),
        CelValue::Array(vec![])
    )]
    #[case::array_basic(
        CelValue::Array(vec![CelValue::Int(1), CelValue::Int(2)]),
        CelValue::Array(vec![CelValue::Int(3), CelValue::Int(4)]),
        CelValue::Array(vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3), CelValue::Int(4)])
    )]
    #[case::array_mixed(
        CelValue::Array(vec![CelValue::Int(1), CelValue::Bool(true)]),
        CelValue::Array(vec![CelValue::Int(2)]),
        CelValue::Array(vec![CelValue::Int(1), CelValue::Bool(true), CelValue::Int(2)])
    )]
    fn test_value_add(#[case] a: CelValue, #[case] b: CelValue, #[case] expected: CelValue) {
        let a_ptr = Box::into_raw(Box::new(a));
        let b_ptr = Box::into_raw(Box::new(b));

        unsafe {
            let result_ptr = cel_value_add(a_ptr, b_ptr);
            let result = &*result_ptr;

            assert_eq!(result, &expected);

            // Clean up
            cel_free_value(a_ptr);
            cel_free_value(b_ptr);
            cel_free_value(result_ptr);
        }
    }
}
