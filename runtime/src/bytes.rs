//! Bytes operations for CEL runtime.
//!
//! This module provides bytes manipulation functions including:
//! - Bytes creation from raw byte sequences
//! - Bytes concatenation
//! - Bytes size (length in bytes)
//! - Bytes comparison (equality and ordering)

use slog::{debug, error};

use crate::{error::abort_with_error, types::CelValue};

/// Internal helper: Concatenate two byte sequences.
///
/// # Arguments
/// - `a`: First byte sequence
/// - `b`: Second byte sequence
///
/// # Returns
/// A new Vec<u8> containing the concatenation of `a` and `b`
pub(crate) fn cel_bytes_concat_internal(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(a.len() + b.len());
    result.extend_from_slice(a);
    result.extend_from_slice(b);
    result
}

/// Creates a CelValue::Bytes from a raw byte sequence.
///
/// # Safety
/// - `data_ptr` must point to valid bytes
/// - `len` must be the correct length of the byte sequence
/// - The caller retains ownership of the input data
///
/// # Arguments
/// - `data_ptr`: Pointer to bytes
/// - `len`: Length of the byte sequence
///
/// # Returns
/// Pointer to a heap-allocated CelValue::Bytes
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_create_bytes(data_ptr: *const u8, len: usize) -> *mut CelValue {
    // Read the bytes from memory
    // SAFETY: Caller guarantees data_ptr is valid and len is correct
    let bytes = unsafe { core::slice::from_raw_parts(data_ptr, len) };

    // Create CelValue::Bytes and allocate on heap
    let value = Box::new(CelValue::Bytes(bytes.to_vec()));
    Box::into_raw(value)
}

/// Returns the size of a bytes value (number of bytes).
///
/// # Panics
/// - If `bytes_ptr` is null
/// - If the CelValue is not a Bytes variant
///
/// # Safety
/// - `bytes_ptr` must be a valid pointer to a CelValue::Bytes
///
/// # Arguments
/// - `bytes_ptr`: Pointer to a CelValue containing bytes
///
/// # Returns
/// The number of bytes in the sequence
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_bytes_size(bytes_ptr: *const CelValue) -> i64 {
    let log = crate::logging::get_logger();

    // Check for null bytes pointer
    if bytes_ptr.is_null() {
        error!(log, "Cannot get size of null bytes";
            "function" => "cel_bytes_size");
        abort_with_error("no such overload");
    }

    // SAFETY: Caller guarantees bytes_ptr is valid
    let value = unsafe { &*bytes_ptr };

    match value {
        CelValue::Bytes(b) => {
            debug!(log, "Getting bytes size"; "length" => b.len());
            b.len() as i64
        }
        _ => {
            error!(log, "Type mismatch in bytes operation";
                "function" => "cel_bytes_size",
                "expected" => "Bytes",
                "actual" => format!("{:?}", value));
            abort_with_error("no such overload")
        }
    }
}

/// Concatenates two bytes values.
///
/// # Panics
/// - If either pointer is null
/// - If either CelValue is not a Bytes variant
///
/// # Safety
/// - Both pointers must be valid CelValue::Bytes pointers
///
/// # Arguments
/// - `left_ptr`: Pointer to the first bytes value
/// - `right_ptr`: Pointer to the second bytes value
///
/// # Returns
/// Pointer to a new CelValue::Bytes containing the concatenation
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_bytes_concat(
    left_ptr: *const CelValue,
    right_ptr: *const CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    // Check for null pointers
    if left_ptr.is_null() {
        error!(log, "Cannot concatenate null bytes (left operand)";
            "function" => "cel_bytes_concat");
        abort_with_error("no such overload");
    }
    if right_ptr.is_null() {
        error!(log, "Cannot concatenate null bytes (right operand)";
            "function" => "cel_bytes_concat");
        abort_with_error("no such overload");
    }

    // SAFETY: Caller guarantees both pointers are valid
    let left_val = unsafe { &*left_ptr };
    let right_val = unsafe { &*right_ptr };

    match (left_val, right_val) {
        (CelValue::Bytes(a), CelValue::Bytes(b)) => {
            debug!(log, "Concatenating bytes"; 
                "left_length" => a.len(), 
                "right_length" => b.len());
            let result = cel_bytes_concat_internal(a, b);
            Box::into_raw(Box::new(CelValue::Bytes(result)))
        }
        _ => {
            error!(log, "Type mismatch in bytes concatenation";
                "function" => "cel_bytes_concat",
                "expected" => "Bytes + Bytes",
                "left_type" => format!("{:?}", left_val),
                "right_type" => format!("{:?}", right_val));
            abort_with_error("no such overload")
        }
    }
}

/// Tests whether two bytes values are equal.
///
/// # Safety
/// - Both pointers must be valid CelValue pointers
///
/// # Arguments
/// - `left_ptr`: Pointer to the first value
/// - `right_ptr`: Pointer to the second value
///
/// # Returns
/// Pointer to CelValue::Bool(true) if bytes are equal, false otherwise
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_bytes_eq(left_ptr: *const CelValue, right_ptr: *const CelValue) -> *mut CelValue {
    // SAFETY: Caller guarantees both pointers are valid
    let left_val = unsafe { &*left_ptr };
    let right_val = unsafe { &*right_ptr };

    let result = match (left_val, right_val) {
        (CelValue::Bytes(a), CelValue::Bytes(b)) => a == b,
        _ => false,
    };

    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Tests whether two bytes values are not equal.
///
/// # Safety
/// - Both pointers must be valid CelValue pointers
///
/// # Arguments
/// - `left_ptr`: Pointer to the first value
/// - `right_ptr`: Pointer to the second value
///
/// # Returns
/// Pointer to CelValue::Bool(true) if bytes are not equal, false otherwise
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_bytes_ne(left_ptr: *const CelValue, right_ptr: *const CelValue) -> *mut CelValue {
    // SAFETY: Caller guarantees both pointers are valid
    let left_val = unsafe { &*left_ptr };
    let right_val = unsafe { &*right_ptr };

    let result = match (left_val, right_val) {
        (CelValue::Bytes(a), CelValue::Bytes(b)) => a != b,
        _ => false,
    };

    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Tests whether first bytes value is less than second (lexicographic ordering).
///
/// # Safety
/// - Both pointers must be valid CelValue::Bytes pointers
///
/// # Arguments
/// - `left_ptr`: Pointer to the first bytes value
/// - `right_ptr`: Pointer to the second bytes value
///
/// # Returns
/// Pointer to CelValue::Bool(true) if left < right, false otherwise
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_bytes_lt(left_ptr: *const CelValue, right_ptr: *const CelValue) -> *mut CelValue {
    // SAFETY: Caller guarantees both pointers are valid
    let left_val = unsafe { &*left_ptr };
    let right_val = unsafe { &*right_ptr };

    let result = match (left_val, right_val) {
        (CelValue::Bytes(a), CelValue::Bytes(b)) => a < b,
        _ => false,
    };

    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Tests whether first bytes value is less than or equal to second (lexicographic ordering).
///
/// # Safety
/// - Both pointers must be valid CelValue::Bytes pointers
///
/// # Arguments
/// - `left_ptr`: Pointer to the first bytes value
/// - `right_ptr`: Pointer to the second bytes value
///
/// # Returns
/// Pointer to CelValue::Bool(true) if left <= right, false otherwise
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_bytes_lte(
    left_ptr: *const CelValue,
    right_ptr: *const CelValue,
) -> *mut CelValue {
    // SAFETY: Caller guarantees both pointers are valid
    let left_val = unsafe { &*left_ptr };
    let right_val = unsafe { &*right_ptr };

    let result = match (left_val, right_val) {
        (CelValue::Bytes(a), CelValue::Bytes(b)) => a <= b,
        _ => false,
    };

    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Tests whether first bytes value is greater than second (lexicographic ordering).
///
/// # Safety
/// - Both pointers must be valid CelValue::Bytes pointers
///
/// # Arguments
/// - `left_ptr`: Pointer to the first bytes value
/// - `right_ptr`: Pointer to the second bytes value
///
/// # Returns
/// Pointer to CelValue::Bool(true) if left > right, false otherwise
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_bytes_gt(left_ptr: *const CelValue, right_ptr: *const CelValue) -> *mut CelValue {
    // SAFETY: Caller guarantees both pointers are valid
    let left_val = unsafe { &*left_ptr };
    let right_val = unsafe { &*right_ptr };

    let result = match (left_val, right_val) {
        (CelValue::Bytes(a), CelValue::Bytes(b)) => a > b,
        _ => false,
    };

    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Tests whether first bytes value is greater than or equal to second (lexicographic ordering).
///
/// # Safety
/// - Both pointers must be valid CelValue::Bytes pointers
///
/// # Arguments
/// - `left_ptr`: Pointer to the first bytes value
/// - `right_ptr`: Pointer to the second bytes value
///
/// # Returns
/// Pointer to CelValue::Bool(true) if left >= right, false otherwise
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_bytes_gte(
    left_ptr: *const CelValue,
    right_ptr: *const CelValue,
) -> *mut CelValue {
    // SAFETY: Caller guarantees both pointers are valid
    let left_val = unsafe { &*left_ptr };
    let right_val = unsafe { &*right_ptr };

    let result = match (left_val, right_val) {
        (CelValue::Bytes(a), CelValue::Bytes(b)) => a >= b,
        _ => false,
    };

    Box::into_raw(Box::new(CelValue::Bool(result)))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn test_create_bytes() {
        let test_bytes = vec![0x48, 0x65, 0x6c, 0x6c, 0x6f]; // "Hello" in ASCII

        unsafe {
            let result_ptr = cel_create_bytes(test_bytes.as_ptr(), test_bytes.len());
            let result = &*result_ptr;

            match result {
                CelValue::Bytes(b) => assert_eq!(b, &test_bytes),
                _ => panic!("Expected Bytes variant"),
            }
        }
    }

    #[test]
    fn test_create_empty_bytes() {
        let test_bytes: Vec<u8> = vec![];

        unsafe {
            let result_ptr = cel_create_bytes(test_bytes.as_ptr(), test_bytes.len());
            let result = &*result_ptr;

            match result {
                CelValue::Bytes(b) => assert_eq!(b, &test_bytes),
                _ => panic!("Expected Bytes variant"),
            }
        }
    }

    #[rstest]
    #[case::basic(vec![1, 2, 3], 3)]
    #[case::empty(vec![], 0)]
    #[case::single(vec![255], 1)]
    fn test_bytes_size(#[case] input: Vec<u8>, #[case] expected: i64) {
        let test_val = CelValue::Bytes(input);

        unsafe {
            let size = cel_bytes_size(&test_val as *const CelValue);
            assert_eq!(size, expected);
        }
    }

    #[rstest]
    #[case::basic(vec![1, 2], vec![3, 4], vec![1, 2, 3, 4])]
    #[case::empty_first(vec![], vec![1, 2], vec![1, 2])]
    #[case::empty_second(vec![1, 2], vec![], vec![1, 2])]
    #[case::both_empty(vec![], vec![], vec![])]
    fn test_bytes_concat(#[case] a: Vec<u8>, #[case] b: Vec<u8>, #[case] expected: Vec<u8>) {
        let result = cel_bytes_concat_internal(&a, &b);
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case::equal(vec![1, 2, 3], vec![1, 2, 3], true)]
    #[case::not_equal(vec![1, 2, 3], vec![1, 2, 4], false)]
    #[case::different_length(vec![1, 2], vec![1, 2, 3], false)]
    #[case::empty(vec![], vec![], true)]
    fn test_bytes_eq(#[case] a: Vec<u8>, #[case] b: Vec<u8>, #[case] expected: bool) {
        let left_val = CelValue::Bytes(a);
        let right_val = CelValue::Bytes(b);

        unsafe {
            let result_ptr =
                cel_bytes_eq(&left_val as *const CelValue, &right_val as *const CelValue);
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
        }
    }

    #[rstest]
    #[case::not_equal(vec![1, 2, 3], vec![1, 2, 4], true)]
    #[case::equal(vec![1, 2, 3], vec![1, 2, 3], false)]
    fn test_bytes_ne(#[case] a: Vec<u8>, #[case] b: Vec<u8>, #[case] expected: bool) {
        let left_val = CelValue::Bytes(a);
        let right_val = CelValue::Bytes(b);

        unsafe {
            let result_ptr =
                cel_bytes_ne(&left_val as *const CelValue, &right_val as *const CelValue);
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
        }
    }

    #[rstest]
    #[case::less(vec![1, 2, 3], vec![1, 2, 4], true)]
    #[case::greater(vec![1, 2, 4], vec![1, 2, 3], false)]
    #[case::equal(vec![1, 2, 3], vec![1, 2, 3], false)]
    #[case::prefix(vec![1, 2], vec![1, 2, 3], true)]
    fn test_bytes_lt(#[case] a: Vec<u8>, #[case] b: Vec<u8>, #[case] expected: bool) {
        let left_val = CelValue::Bytes(a);
        let right_val = CelValue::Bytes(b);

        unsafe {
            let result_ptr =
                cel_bytes_lt(&left_val as *const CelValue, &right_val as *const CelValue);
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
        }
    }

    #[rstest]
    #[case::less(vec![1, 2, 3], vec![1, 2, 4], true)]
    #[case::equal(vec![1, 2, 3], vec![1, 2, 3], true)]
    #[case::greater(vec![1, 2, 4], vec![1, 2, 3], false)]
    fn test_bytes_lte(#[case] a: Vec<u8>, #[case] b: Vec<u8>, #[case] expected: bool) {
        let left_val = CelValue::Bytes(a);
        let right_val = CelValue::Bytes(b);

        unsafe {
            let result_ptr =
                cel_bytes_lte(&left_val as *const CelValue, &right_val as *const CelValue);
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
        }
    }

    #[rstest]
    #[case::greater(vec![1, 2, 4], vec![1, 2, 3], true)]
    #[case::less(vec![1, 2, 3], vec![1, 2, 4], false)]
    #[case::equal(vec![1, 2, 3], vec![1, 2, 3], false)]
    fn test_bytes_gt(#[case] a: Vec<u8>, #[case] b: Vec<u8>, #[case] expected: bool) {
        let left_val = CelValue::Bytes(a);
        let right_val = CelValue::Bytes(b);

        unsafe {
            let result_ptr =
                cel_bytes_gt(&left_val as *const CelValue, &right_val as *const CelValue);
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
        }
    }

    #[rstest]
    #[case::greater(vec![1, 2, 4], vec![1, 2, 3], true)]
    #[case::equal(vec![1, 2, 3], vec![1, 2, 3], true)]
    #[case::less(vec![1, 2, 3], vec![1, 2, 4], false)]
    fn test_bytes_gte(#[case] a: Vec<u8>, #[case] b: Vec<u8>, #[case] expected: bool) {
        let left_val = CelValue::Bytes(a);
        let right_val = CelValue::Bytes(b);

        unsafe {
            let result_ptr =
                cel_bytes_gte(&left_val as *const CelValue, &right_val as *const CelValue);
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
        }
    }
}
