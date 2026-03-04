//! Comparison operations returning CelValue::Bool pointers.
//!
//! Following CEL specification:
//! - Integer comparisons: standard equality and ordering
//! - Double comparisons: IEEE 754 semantics with proper NaN handling
//!   - NaN != NaN is true (per IEEE 754)
//!   - NaN comparisons with other values return false (except !=)

use crate::helpers::{cel_create_bool, extract_double, extract_int, extract_uint};
use crate::types::CelValue;

/// Helper function to check if either operand is an error and propagate it.
/// Returns Some(error_ptr) if either is an error, None otherwise.
fn check_for_errors(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> Option<*mut CelValue> {
    unsafe {
        if !a_ptr.is_null()
            && let CelValue::Error(_) = &*a_ptr
        {
            return Some(a_ptr);
        }
        if !b_ptr.is_null()
            && let CelValue::Error(_) = &*b_ptr
        {
            return Some(b_ptr);
        }
    }
    None
}

/// Compares two integers for equality.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Int instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_int_eq(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    if let Some(err) = check_for_errors(a_ptr, b_ptr) {
        return err;
    }
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a == b { 1 } else { 0 })
}

/// Compares two integers for inequality.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Int instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_int_ne(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    if let Some(err) = check_for_errors(a_ptr, b_ptr) {
        return err;
    }
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a != b { 1 } else { 0 })
}

/// Tests if first integer is greater than second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Int instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_int_gt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    if let Some(err) = check_for_errors(a_ptr, b_ptr) {
        return err;
    }
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a > b { 1 } else { 0 })
}

/// Tests if first integer is less than second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Int instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_int_lt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    if let Some(err) = check_for_errors(a_ptr, b_ptr) {
        return err;
    }
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a < b { 1 } else { 0 })
}

/// Tests if first integer is greater than or equal to second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Int instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_int_gte(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    if let Some(err) = check_for_errors(a_ptr, b_ptr) {
        return err;
    }
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a >= b { 1 } else { 0 })
}

/// Tests if first integer is less than or equal to second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Int instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_int_lte(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    if let Some(err) = check_for_errors(a_ptr, b_ptr) {
        return err;
    }
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a <= b { 1 } else { 0 })
}

// Unsigned integer comparison operations

/// Compares two unsigned integers for equality.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::UInt instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint_eq(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_uint(a_ptr);
    let b = extract_uint(b_ptr);
    cel_create_bool(if a == b { 1 } else { 0 })
}

/// Compares two unsigned integers for inequality.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::UInt instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint_ne(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_uint(a_ptr);
    let b = extract_uint(b_ptr);
    cel_create_bool(if a != b { 1 } else { 0 })
}

/// Tests if first unsigned integer is greater than second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::UInt instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint_gt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_uint(a_ptr);
    let b = extract_uint(b_ptr);
    cel_create_bool(if a > b { 1 } else { 0 })
}

/// Tests if first unsigned integer is less than second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::UInt instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint_lt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_uint(a_ptr);
    let b = extract_uint(b_ptr);
    cel_create_bool(if a < b { 1 } else { 0 })
}

/// Tests if first unsigned integer is greater than or equal to second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::UInt instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint_gte(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_uint(a_ptr);
    let b = extract_uint(b_ptr);
    cel_create_bool(if a >= b { 1 } else { 0 })
}

/// Tests if first unsigned integer is less than or equal to second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::UInt instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint_lte(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_uint(a_ptr);
    let b = extract_uint(b_ptr);
    cel_create_bool(if a <= b { 1 } else { 0 })
}

// Double comparison operations
// Note: These follow IEEE 754 semantics where NaN != NaN is true

/// Compares two doubles for equality (IEEE 754 semantics).
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Double instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_double_eq(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    cel_create_bool(if a == b { 1 } else { 0 })
}

/// Compares two doubles for inequality (IEEE 754 semantics).
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Double instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_double_ne(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    cel_create_bool(if a != b { 1 } else { 0 })
}

/// Tests if first double is greater than second (IEEE 754 semantics).
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Double instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_double_gt(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    cel_create_bool(if a > b { 1 } else { 0 })
}

/// Tests if first double is less than second (IEEE 754 semantics).
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Double instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_double_lt(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    cel_create_bool(if a < b { 1 } else { 0 })
}

/// Tests if first double is greater than or equal to second (IEEE 754 semantics).
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Double instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_double_gte(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    cel_create_bool(if a >= b { 1 } else { 0 })
}

/// Tests if first double is less than or equal to second (IEEE 754 semantics).
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Double instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_double_lte(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    cel_create_bool(if a <= b { 1 } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // Helper function to test double comparisons
    // Reduces boilerplate for pointer creation, unsafe dereferencing, and cleanup
    fn assert_double_comparison(
        a: f64,
        b: f64,
        op: unsafe extern "C" fn(*mut CelValue, *mut CelValue) -> *mut CelValue,
        expected: bool,
    ) {
        unsafe {
            let a_ptr = crate::helpers::cel_create_double(a);
            let b_ptr = crate::helpers::cel_create_double(b);

            let result_ptr = op(a_ptr, b_ptr);
            let result = match &*result_ptr {
                CelValue::Bool(b) => *b,
                _ => panic!("Expected Bool"),
            };
            assert_eq!(result, expected);

            let _ = Box::from_raw(a_ptr);
            let _ = Box::from_raw(b_ptr);
            let _ = Box::from_raw(result_ptr);
        }
    }

    // Double comparison tests with IEEE 754 semantics

    #[rstest]
    #[case::eq_same(3.14, 3.14, true)]
    #[case::eq_different(3.14, 2.71, false)]
    #[case::eq_negative(-5.0, -5.0, true)]
    #[case::eq_nan(f64::NAN, f64::NAN, false)] // IEEE 754: NaN != NaN
    fn test_double_eq(#[case] a: f64, #[case] b: f64, #[case] expected: bool) {
        assert_double_comparison(a, b, cel_double_eq, expected);
    }

    #[rstest]
    #[case::ne_same(3.14, 3.14, false)]
    #[case::ne_different(3.14, 2.71, true)]
    #[case::ne_nan(f64::NAN, f64::NAN, true)] // IEEE 754: NaN != NaN is true
    fn test_double_ne(#[case] a: f64, #[case] b: f64, #[case] expected: bool) {
        assert_double_comparison(a, b, cel_double_ne, expected);
    }

    #[rstest]
    #[case::gt_true(5.0, 3.0, true)]
    #[case::gt_false(3.0, 5.0, false)]
    #[case::gt_equal(3.0, 3.0, false)]
    #[case::gt_infinity(f64::INFINITY, 5.0, true)]
    #[case::gt_neg_infinity(5.0, f64::NEG_INFINITY, true)]
    fn test_double_gt(#[case] a: f64, #[case] b: f64, #[case] expected: bool) {
        assert_double_comparison(a, b, cel_double_gt, expected);
    }

    #[rstest]
    #[case::lt_true(3.0, 5.0, true)]
    #[case::lt_false(5.0, 3.0, false)]
    #[case::lt_equal(3.0, 3.0, false)]
    #[case::lt_infinity(5.0, f64::INFINITY, true)]
    #[case::lt_neg_infinity(f64::NEG_INFINITY, 5.0, true)]
    fn test_double_lt(#[case] a: f64, #[case] b: f64, #[case] expected: bool) {
        assert_double_comparison(a, b, cel_double_lt, expected);
    }

    #[rstest]
    #[case::gte_greater(5.0, 3.0, true)]
    #[case::gte_equal(3.0, 3.0, true)]
    #[case::gte_less(3.0, 5.0, false)]
    fn test_double_gte(#[case] a: f64, #[case] b: f64, #[case] expected: bool) {
        assert_double_comparison(a, b, cel_double_gte, expected);
    }

    #[rstest]
    #[case::lte_less(3.0, 5.0, true)]
    #[case::lte_equal(3.0, 3.0, true)]
    #[case::lte_greater(5.0, 3.0, false)]
    fn test_double_lte(#[case] a: f64, #[case] b: f64, #[case] expected: bool) {
        assert_double_comparison(a, b, cel_double_lte, expected);
    }

    // Uint comparison tests

    fn assert_uint_comparison(
        a: u64,
        b: u64,
        op: unsafe extern "C" fn(*mut CelValue, *mut CelValue) -> *mut CelValue,
        expected: bool,
    ) {
        unsafe {
            let a_ptr = crate::helpers::cel_create_uint(a);
            let b_ptr = crate::helpers::cel_create_uint(b);

            let result_ptr = op(a_ptr, b_ptr);
            let result = match &*result_ptr {
                CelValue::Bool(b) => *b,
                _ => panic!("Expected Bool"),
            };
            assert_eq!(result, expected);

            let _ = Box::from_raw(a_ptr);
            let _ = Box::from_raw(b_ptr);
            let _ = Box::from_raw(result_ptr);
        }
    }

    #[rstest]
    #[case::eq_same(100, 100, true)]
    #[case::eq_different(100, 200, false)]
    #[case::eq_zero(0, 0, true)]
    #[case::eq_max(u64::MAX, u64::MAX, true)]
    fn test_uint_eq(#[case] a: u64, #[case] b: u64, #[case] expected: bool) {
        assert_uint_comparison(a, b, cel_uint_eq, expected);
    }

    #[rstest]
    #[case::ne_same(100, 100, false)]
    #[case::ne_different(100, 200, true)]
    #[case::ne_zero_one(0, 1, true)]
    fn test_uint_ne(#[case] a: u64, #[case] b: u64, #[case] expected: bool) {
        assert_uint_comparison(a, b, cel_uint_ne, expected);
    }

    #[rstest]
    #[case::gt_true(100, 50, true)]
    #[case::gt_false(50, 100, false)]
    #[case::gt_equal(100, 100, false)]
    #[case::gt_max(u64::MAX, 0, true)]
    fn test_uint_gt(#[case] a: u64, #[case] b: u64, #[case] expected: bool) {
        assert_uint_comparison(a, b, cel_uint_gt, expected);
    }

    #[rstest]
    #[case::lt_true(50, 100, true)]
    #[case::lt_false(100, 50, false)]
    #[case::lt_equal(100, 100, false)]
    #[case::lt_zero_max(0, u64::MAX, true)]
    fn test_uint_lt(#[case] a: u64, #[case] b: u64, #[case] expected: bool) {
        assert_uint_comparison(a, b, cel_uint_lt, expected);
    }

    #[rstest]
    #[case::gte_greater(100, 50, true)]
    #[case::gte_equal(100, 100, true)]
    #[case::gte_less(50, 100, false)]
    fn test_uint_gte(#[case] a: u64, #[case] b: u64, #[case] expected: bool) {
        assert_uint_comparison(a, b, cel_uint_gte, expected);
    }

    #[rstest]
    #[case::lte_less(50, 100, true)]
    #[case::lte_equal(100, 100, true)]
    #[case::lte_greater(100, 50, false)]
    fn test_uint_lte(#[case] a: u64, #[case] b: u64, #[case] expected: bool) {
        assert_uint_comparison(a, b, cel_uint_lte, expected);
    }
}

// Timestamp comparison functions
use crate::helpers::extract_timestamp;

/// Tests if first timestamp is less than second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Timestamp instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_lt(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_timestamp(a_ptr);
    let b = extract_timestamp(b_ptr);
    cel_create_bool(if a < b { 1 } else { 0 })
}

/// Tests if first timestamp is less than or equal to second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Timestamp instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_lte(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_timestamp(a_ptr);
    let b = extract_timestamp(b_ptr);
    cel_create_bool(if a <= b { 1 } else { 0 })
}

/// Tests if first timestamp is greater than second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Timestamp instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_gt(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_timestamp(a_ptr);
    let b = extract_timestamp(b_ptr);
    cel_create_bool(if a > b { 1 } else { 0 })
}

/// Tests if first timestamp is greater than or equal to second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Timestamp instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_gte(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_timestamp(a_ptr);
    let b = extract_timestamp(b_ptr);
    cel_create_bool(if a >= b { 1 } else { 0 })
}

// Duration comparison functions
use crate::helpers::extract_duration_chrono;

/// Tests if first duration is less than second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Duration instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_duration_lt(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_duration_chrono(a_ptr);
    let b = extract_duration_chrono(b_ptr);
    cel_create_bool(if a < b { 1 } else { 0 })
}

/// Tests if first duration is less than or equal to second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Duration instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_duration_lte(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_duration_chrono(a_ptr);
    let b = extract_duration_chrono(b_ptr);
    cel_create_bool(if a <= b { 1 } else { 0 })
}

/// Tests if first duration is greater than second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Duration instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_duration_gt(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_duration_chrono(a_ptr);
    let b = extract_duration_chrono(b_ptr);
    cel_create_bool(if a > b { 1 } else { 0 })
}

/// Tests if first duration is greater than or equal to second.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Duration instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_duration_gte(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_duration_chrono(a_ptr);
    let b = extract_duration_chrono(b_ptr);
    cel_create_bool(if a >= b { 1 } else { 0 })
}
