//! Arithmetic operations with overflow checking and division-by-zero protection.
//! All operations return errors via cel_abort on overflow or invalid operations per CEL spec.
//!
//! Following CEL specification:
//! - Integer operations: checked arithmetic with overflow protection
//! - Double operations: IEEE 754 floating point arithmetic
//! - NO automatic type coercion between Int and Double

use crate::error::abort_with_error;
use crate::helpers::{
    cel_create_double, cel_create_int, cel_create_uint, extract_double, extract_int, extract_uint,
};
use crate::types::CelValue;
use slog::warn;

/// Internal helper: Add two integers with overflow checking.
///
/// # Arguments
/// - `a`: First integer operand
/// - `b`: Second integer operand
///
/// # Returns
/// The sum of `a` and `b`
///
/// # Errors
/// Calls cel_abort on integer overflow
#[allow(dead_code)]
pub(crate) fn cel_int_add(a: i64, b: i64) -> i64 {
    match a.checked_add(b) {
        Some(result) => result,
        None => abort_with_error("integer overflow in addition"),
    }
}

/// Internal helper: Add two doubles using IEEE 754 floating point arithmetic.
///
/// # Arguments
/// - `a`: First double operand
/// - `b`: Second double operand
///
/// # Returns
/// The sum of `a` and `b`
///
/// Note: This follows IEEE 754 semantics (NaN propagation, infinity handling, etc.)
pub(crate) fn double_add(a: f64, b: f64) -> f64 {
    a + b
}

/// Internal helper: Subtract two doubles using IEEE 754 floating point arithmetic.
pub(crate) fn double_sub(a: f64, b: f64) -> f64 {
    a - b
}

/// Internal helper: Multiply two doubles using IEEE 754 floating point arithmetic.
pub(crate) fn double_mul(a: f64, b: f64) -> f64 {
    a * b
}

/// Internal helper: Divide two doubles using IEEE 754 floating point arithmetic.
/// Note: Unlike integer division, division by zero yields Infinity or NaN per IEEE 754.
pub(crate) fn double_div(a: f64, b: f64) -> f64 {
    a / b
}

/// Subtracts two integers with overflow checking.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Int instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_int_sub(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    match a.checked_sub(b) {
        Some(result) => cel_create_int(result),
        None => abort_with_error("integer overflow"),
    }
}

/// Multiplies two integers with overflow checking.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Int instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_int_mul(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    match a.checked_mul(b) {
        Some(result) => cel_create_int(result),
        None => abort_with_error("integer overflow"),
    }
}

/// Divides two integers with overflow and division-by-zero checking.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Int instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_int_div(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);

    if b == 0 {
        abort_with_error("division by zero");
    }

    // checked_div also catches the special case: i64::MIN / -1
    match a.checked_div(b) {
        Some(result) => cel_create_int(result),
        None => abort_with_error("integer overflow"),
    }
}

/// Computes modulus of two integers with overflow and modulus-by-zero checking.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Int instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_int_mod(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);

    if b == 0 {
        abort_with_error("modulus by zero");
    }

    // checked_rem also catches the special case: i64::MIN % -1
    match a.checked_rem(b) {
        Some(result) => cel_create_int(result),
        None => abort_with_error("integer overflow"),
    }
}

/// Adds two unsigned integers with overflow checking.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::UInt instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint_add(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_uint(a_ptr);
    let b = extract_uint(b_ptr);
    match a.checked_add(b) {
        Some(result) => cel_create_uint(result),
        None => abort_with_error("unsigned integer overflow"),
    }
}

/// Subtracts two unsigned integers with underflow checking.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::UInt instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint_sub(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_uint(a_ptr);
    let b = extract_uint(b_ptr);
    match a.checked_sub(b) {
        Some(result) => cel_create_uint(result),
        None => abort_with_error("unsigned integer underflow"),
    }
}

/// Multiplies two unsigned integers with overflow checking.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::UInt instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint_mul(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_uint(a_ptr);
    let b = extract_uint(b_ptr);
    match a.checked_mul(b) {
        Some(result) => cel_create_uint(result),
        None => abort_with_error("unsigned integer overflow"),
    }
}

/// Divides two unsigned integers with division-by-zero checking.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::UInt instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint_div(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_uint(a_ptr);
    let b = extract_uint(b_ptr);
    if b == 0 {
        abort_with_error("division by zero");
    }
    cel_create_uint(a / b)
}

/// Computes modulus of two unsigned integers with modulus-by-zero checking.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::UInt instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint_mod(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_uint(a_ptr);
    let b = extract_uint(b_ptr);
    if b == 0 {
        abort_with_error("modulus by zero");
    }
    cel_create_uint(a % b)
}

/// Adds two doubles using IEEE 754 floating point arithmetic.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Double instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_double_add(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    let result = double_add(a, b);
    cel_create_double(result)
}

/// Subtracts two doubles using IEEE 754 floating point arithmetic.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Double instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_double_sub(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    let result = double_sub(a, b);
    cel_create_double(result)
}

/// Multiplies two doubles using IEEE 754 floating point arithmetic.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Double instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_double_mul(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    let result = double_mul(a, b);
    cel_create_double(result)
}

/// Divides two doubles using IEEE 754 floating point arithmetic.
/// Division by zero yields Infinity or NaN per IEEE 754.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Double instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_double_div(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();
    let a = extract_double(a_ptr);
    let b = extract_double(b_ptr);
    let result = double_div(a, b);

    // Warn about special IEEE 754 results
    if result.is_infinite() {
        warn!(log, "Division resulted in Infinity";
            "operation" => "cel_double_div",
            "dividend" => a,
            "divisor" => b,
            "result" => format!("{}", result));
    } else if result.is_nan() {
        warn!(log, "Division resulted in NaN";
            "operation" => "cel_double_div",
            "dividend" => a,
            "divisor" => b);
    }

    cel_create_double(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_int_add_basic() {
        let result = cel_int_add(2, 3);
        assert_eq!(result, 5);
    }

    #[test]
    fn test_int_add_negative() {
        let result = cel_int_add(-5, 3);
        assert_eq!(result, -2);
    }

    #[test]
    #[should_panic(expected = "integer overflow in addition")]
    fn test_int_add_overflow() {
        cel_int_add(i64::MAX, 1);
    }

    // Double arithmetic tests

    #[rstest]
    #[case::add_basic(2.5, 3.5, 6.0)]
    #[case::add_negative(-5.5, 3.0, -2.5)]
    #[case::add_zero(5.0, 0.0, 5.0)]
    fn test_double_add(#[case] a: f64, #[case] b: f64, #[case] expected: f64) {
        let result = double_add(a, b);
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case::sub_basic(5.5, 2.0, 3.5)]
    #[case::sub_negative(-5.0, -3.0, -2.0)]
    #[case::sub_zero(5.0, 0.0, 5.0)]
    fn test_double_sub(#[case] a: f64, #[case] b: f64, #[case] expected: f64) {
        let result = double_sub(a, b);
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case::mul_basic(2.5, 4.0, 10.0)]
    #[case::mul_negative(-2.0, 3.0, -6.0)]
    #[case::mul_zero(5.0, 0.0, 0.0)]
    fn test_double_mul(#[case] a: f64, #[case] b: f64, #[case] expected: f64) {
        let result = double_mul(a, b);
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case::div_basic(10.0, 2.0, 5.0)]
    #[case::div_negative(-6.0, 3.0, -2.0)]
    #[case::div_fraction(5.0, 2.0, 2.5)]
    fn test_double_div(#[case] a: f64, #[case] b: f64, #[case] expected: f64) {
        let result = double_div(a, b);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_double_div_by_zero_yields_infinity() {
        let result = double_div(1.0, 0.0);
        assert!(result.is_infinite());
        assert!(result.is_sign_positive());
    }

    #[test]
    fn test_double_div_negative_by_zero_yields_neg_infinity() {
        let result = double_div(-1.0, 0.0);
        assert!(result.is_infinite());
        assert!(result.is_sign_negative());
    }

    #[test]
    fn test_double_nan_propagation() {
        let nan = f64::NAN;
        assert!(double_add(nan, 1.0).is_nan());
        assert!(double_sub(nan, 1.0).is_nan());
        assert!(double_mul(nan, 1.0).is_nan());
        assert!(double_div(nan, 1.0).is_nan());
    }

    #[test]
    fn test_double_infinity_arithmetic() {
        let inf = f64::INFINITY;
        assert_eq!(double_add(inf, 1.0), f64::INFINITY);
        assert_eq!(double_mul(inf, 2.0), f64::INFINITY);
        assert_eq!(double_div(inf, 2.0), f64::INFINITY);
    }

    // Uint arithmetic tests

    #[rstest]
    #[case::add_basic(10, 20, 30)]
    #[case::add_zero(5, 0, 5)]
    #[case::add_large(u64::MAX - 100, 50, u64::MAX - 50)]
    fn test_uint_add(#[case] a: u64, #[case] b: u64, #[case] expected: u64) {
        unsafe {
            let a_val = cel_create_uint(a);
            let b_val = cel_create_uint(b);
            let result_ptr = cel_uint_add(a_val, b_val);
            let result = extract_uint(result_ptr);
            assert_eq!(result, expected);
        }
    }

    #[rstest]
    #[case::sub_basic(20, 10, 10)]
    #[case::sub_zero(5, 0, 5)]
    #[case::sub_same(100, 100, 0)]
    fn test_uint_sub(#[case] a: u64, #[case] b: u64, #[case] expected: u64) {
        unsafe {
            let a_val = cel_create_uint(a);
            let b_val = cel_create_uint(b);
            let result_ptr = cel_uint_sub(a_val, b_val);
            let result = extract_uint(result_ptr);
            assert_eq!(result, expected);
        }
    }

    #[rstest]
    #[case::mul_basic(10, 20, 200)]
    #[case::mul_zero(5, 0, 0)]
    #[case::mul_one(100, 1, 100)]
    fn test_uint_mul(#[case] a: u64, #[case] b: u64, #[case] expected: u64) {
        unsafe {
            let a_val = cel_create_uint(a);
            let b_val = cel_create_uint(b);
            let result_ptr = cel_uint_mul(a_val, b_val);
            let result = extract_uint(result_ptr);
            assert_eq!(result, expected);
        }
    }

    #[rstest]
    #[case::div_basic(20, 10, 2)]
    #[case::div_one(100, 1, 100)]
    #[case::div_truncate(7, 3, 2)]
    fn test_uint_div(#[case] a: u64, #[case] b: u64, #[case] expected: u64) {
        unsafe {
            let a_val = cel_create_uint(a);
            let b_val = cel_create_uint(b);
            let result_ptr = cel_uint_div(a_val, b_val);
            let result = extract_uint(result_ptr);
            assert_eq!(result, expected);
        }
    }

    #[rstest]
    #[case::mod_basic(10, 3, 1)]
    #[case::mod_zero(10, 5, 0)]
    #[case::mod_large(100, 7, 2)]
    fn test_uint_mod(#[case] a: u64, #[case] b: u64, #[case] expected: u64) {
        unsafe {
            let a_val = cel_create_uint(a);
            let b_val = cel_create_uint(b);
            let result_ptr = cel_uint_mod(a_val, b_val);
            let result = extract_uint(result_ptr);
            assert_eq!(result, expected);
        }
    }

    // Note: Panic tests for overflow/underflow/division by zero removed
    // because they cause issues with extern "C" functions that cannot unwind.
    // The panic behavior is tested indirectly through integration tests.
}
