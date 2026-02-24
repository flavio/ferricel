//! Comparison operations returning CelValue::Bool pointers.
//!
//! Following CEL specification:
//! - Integer comparisons: standard equality and ordering
//! - Double comparisons: IEEE 754 semantics with proper NaN handling
//!   - NaN != NaN is true (per IEEE 754)
//!   - NaN comparisons with other values return false (except !=)

use crate::helpers::{cel_create_bool, extract_double, extract_int};
use crate::types::CelValue;

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_eq(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a == b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_ne(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a != b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_gt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a > b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_lt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a < b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_gte(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a >= b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_lte(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a <= b { 1 } else { 0 })
}

// Double comparison operations
// Note: These follow IEEE 754 semantics where NaN != NaN is true

#[unsafe(no_mangle)]
pub extern "C" fn cel_double_eq(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    cel_create_bool(if a == b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_double_ne(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    cel_create_bool(if a != b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_double_gt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    cel_create_bool(if a > b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_double_lt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    cel_create_bool(if a < b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_double_gte(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    cel_create_bool(if a >= b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_double_lte(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
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
        let a_ptr = crate::helpers::cel_create_double(a);
        let b_ptr = crate::helpers::cel_create_double(b);

        unsafe {
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
}
