//! Helper functions for creating and extracting CelValue pointers.
//! These are used internally by other runtime functions and exported for compiler use.

extern crate alloc;
use crate::types::CelValue;
use alloc::boxed::Box;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
