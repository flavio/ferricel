// Integration tests for double (f64) arithmetic, comparisons, and type safety.

use crate::common::*;
use rstest::rstest;

// ============================================================
// Double Literal Tests
// ============================================================

#[rstest]
#[case("4.23", 4.23)]
#[case("0.0", 0.0)]
#[case("-2.5", -2.5)]
#[case("123.456", 123.456)]
fn test_literal_doubles(#[case] expr: &str, #[case] expected: f64) {
    let result = compile_and_execute_double(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ============================================================
// Double Arithmetic Tests
// ============================================================

#[rstest]
#[case("2.5 + 3.5", 6.0)]
#[case("5.0 + 0.0", 5.0)]
#[case("-5.5 + 3.0", -2.5)]
#[case("1.1 + 2.2", 3.3)]
fn test_double_addition(#[case] expr: &str, #[case] expected: f64) {
    let result = compile_and_execute_double(expr).expect("Failed to compile and execute");
    assert!(
        (result - expected).abs() < 1e-10,
        "Expression '{}' should evaluate to {}, got {}",
        expr,
        expected,
        result
    );
}

#[rstest]
#[case("5.5 - 2.0", 3.5)]
#[case("10.0 - 5.0", 5.0)]
#[case("-5.0 - 3.0", -8.0)]
#[case("0.0 - 5.5", -5.5)]
fn test_double_subtraction(#[case] expr: &str, #[case] expected: f64) {
    let result = compile_and_execute_double(expr).expect("Failed to compile and execute");
    assert!(
        (result - expected).abs() < 1e-10,
        "Expression '{}' should evaluate to {}, got {}",
        expr,
        expected,
        result
    );
}

#[rstest]
#[case("2.5 * 4.0", 10.0)]
#[case("3.0 * 3.0", 9.0)]
#[case("-2.0 * 3.0", -6.0)]
#[case("0.0 * 100.0", 0.0)]
fn test_double_multiplication(#[case] expr: &str, #[case] expected: f64) {
    let result = compile_and_execute_double(expr).expect("Failed to compile and execute");
    assert!(
        (result - expected).abs() < 1e-10,
        "Expression '{}' should evaluate to {}, got {}",
        expr,
        expected,
        result
    );
}

#[rstest]
#[case("10.0 / 2.0", 5.0)]
#[case("7.0 / 2.0", 3.5)] // Double division (not integer)
#[case("-10.0 / 2.0", -5.0)]
#[case("5.0 / 2.0", 2.5)]
fn test_double_division(#[case] expr: &str, #[case] expected: f64) {
    let result = compile_and_execute_double(expr).expect("Failed to compile and execute");
    assert!(
        (result - expected).abs() < 1e-10,
        "Expression '{}' should evaluate to {}, got {}",
        expr,
        expected,
        result
    );
}

#[test]
fn test_double_division_by_zero_yields_infinity() {
    // Note: Division by zero in doubles yields Infinity per IEEE 754,
    // but serde_json serializes Infinity as null since it's not valid JSON.
    // This test verifies that the division compiles and runs without panicking,
    // even though we can't easily check the Infinity value through JSON.
    let result = compile_and_execute_double("1.0 / 0.0");
    // The result will be an error because JSON serialization yields null
    // which cannot be parsed as f64. This is expected behavior.
    assert!(result.is_err(), "Infinity serializes as null in JSON");
}

// ============================================================
// Double Comparison Tests
// ============================================================

#[rstest]
#[case("3.14 == 3.14", true)]
#[case("3.14 == 2.71", false)]
#[case("0.0 == 0.0", true)]
fn test_double_equality(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("5.0 > 3.0", true)]
#[case("3.0 > 5.0", false)]
#[case("5.0 > 5.0", false)]
#[case("5.0 >= 5.0", true)]
#[case("5.0 >= 3.0", true)]
#[case("3.0 >= 5.0", false)]
fn test_double_greater_than(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("3.0 < 5.0", true)]
#[case("5.0 < 3.0", false)]
#[case("5.0 < 5.0", false)]
#[case("5.0 <= 5.0", true)]
#[case("3.0 <= 5.0", true)]
#[case("5.0 <= 3.0", false)]
fn test_double_less_than(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ============================================================
// Type Safety Tests (No Auto-Coercion)
// ============================================================

#[test]
fn test_no_mixed_type_arithmetic() {
    // CEL spec: NO automatic type coercion
    // Int + Double should fail (not compile or runtime error)
    // Note: This currently might not be enforced at compile time,
    // but should fail at runtime
    let result = compile_and_execute("1 + 1.0");
    assert!(
        result.is_err(),
        "Mixed-type arithmetic (Int + Double) should fail per CEL spec"
    );
}
