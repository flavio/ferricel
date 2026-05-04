//! Arithmetic operations with overflow checking and division-by-zero protection.
//! All operations return errors via cel_abort on overflow or invalid operations per CEL spec.
//!
//! Following CEL specification:
//! - Integer operations: checked arithmetic with overflow protection
//! - Double operations: IEEE 754 floating point arithmetic
//! - NO automatic type coercion between Int and Double

use crate::error::abort_with_error;

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

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

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

    // Note: Panic tests for overflow/underflow/division by zero removed
    // because they cause issues with extern "C" functions that cannot unwind.
    // The panic behavior is tested indirectly through integration tests.
}
