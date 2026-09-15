//! Bytes operations for CEL runtime.
//!
//! This module provides bytes manipulation functions including:
//! - Bytes creation from raw byte sequences
//! - Bytes concatenation
//! - Bytes size (length in bytes)
//! - Bytes comparison (equality and ordering)

use crate::types::CelValue;

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
    #[case::basic(vec![1, 2], vec![3, 4], vec![1, 2, 3, 4])]
    #[case::empty_first(vec![], vec![1, 2], vec![1, 2])]
    #[case::empty_second(vec![1, 2], vec![], vec![1, 2])]
    #[case::both_empty(vec![], vec![], vec![])]
    fn test_bytes_concat(#[case] a: Vec<u8>, #[case] b: Vec<u8>, #[case] expected: Vec<u8>) {
        let result = cel_bytes_concat_internal(&a, &b);
        assert_eq!(result, expected);
    }
}
