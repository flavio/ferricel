// Integration tests for integer arithmetic and comparison operators.

mod common;
use common::*;
use rstest::rstest;

// ============================================================
// Literal Integer Tests
// ============================================================

#[rstest]
#[case("42", 42)]
#[case("0", 0)]
#[case("1", 1)]
#[case("-5", -5)]
#[case("9999", 9999)]
fn test_literal_integers(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ============================================================
// Addition Tests
// ============================================================

#[rstest]
#[case("1 + 1", 2)]
#[case("10 + 20", 30)]
#[case("5 + 7", 12)]
#[case("100 + 200", 300)]
#[case("0 + 0", 0)]
#[case("-5 + 10", 5)]
#[case("10 + -5", 5)]
fn test_simple_addition(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("1 + 2 + 3", 6)]
#[case("10 + 20 + 30", 60)]
#[case("1 + 2 + 3 + 4 + 5", 15)]
#[case("100 + 200 + 300", 600)]
#[case("1 + 1 + 1 + 1 + 1 + 1", 6)]
#[case("10 + 20 + 30 + 40 + 50", 150)]
fn test_chained_addition(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("(10 + 20)", 30)]
#[case("((5 + 5))", 10)]
#[case("(1 + 2) + 3", 6)]
#[case("1 + (2 + 3)", 6)]
#[case("(1 + 2) + (3 + 4)", 10)]
fn test_parenthesized_expressions(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("0 + 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9", 45)]
#[case("100 + 200 + 300 + 400 + 500", 1500)]
fn test_large_expressions(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ============================================================
// Subtraction Tests
// ============================================================

#[rstest]
#[case("10 - 5", 5)]
#[case("100 - 50", 50)]
#[case("5 - 10", -5)]
#[case("0 - 5", -5)]
#[case("10 - 0", 10)]
#[case("-5 - 10", -15)]
#[case("10 - -5", 15)]
fn test_subtraction(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ============================================================
// Multiplication Tests
// ============================================================

#[rstest]
#[case("2 * 3", 6)]
#[case("5 * 5", 25)]
#[case("10 * 10", 100)]
#[case("0 * 100", 0)]
#[case("100 * 0", 0)]
#[case("-5 * 3", -15)]
#[case("5 * -3", -15)]
#[case("-5 * -3", 15)]
fn test_multiplication(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ============================================================
// Division Tests
// ============================================================

#[rstest]
#[case("10 / 2", 5)]
#[case("100 / 10", 10)]
#[case("7 / 2", 3)] // Integer division
#[case("0 / 5", 0)]
#[case("-10 / 2", -5)]
#[case("10 / -2", -5)]
#[case("-10 / -2", 5)]
fn test_division(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[test]
fn test_division_by_zero() {
    let result = compile_and_execute("10 / 0");
    assert!(
        result.is_err(),
        "Division by zero should produce an error, got: {:?}",
        result
    );
}

// ============================================================
// Modulo Tests
// ============================================================

#[rstest]
#[case("10 % 3", 1)]
#[case("100 % 7", 2)]
#[case("5 % 5", 0)]
#[case("3 % 10", 3)]
#[case("0 % 5", 0)]
fn test_modulo(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[test]
fn test_modulo_by_zero() {
    let result = compile_and_execute("10 % 0");
    assert!(
        result.is_err(),
        "Modulo by zero should produce an error, got: {:?}",
        result
    );
}

// ============================================================
// Mixed Arithmetic Tests
// ============================================================

#[rstest]
#[case("2 + 3 * 4", 14)] // CEL respects precedence: 3*4 first, then +2
#[case("10 - 2 * 3", 4)] // 2*3 first, then 10-6
#[case("20 / 4 + 3", 8)] // 20/4 first, then +3
#[case("(2 + 3) * 4", 20)] // Parentheses override precedence
#[case("10 * 2 + 5 * 3", 35)] // 10*2 + 5*3 = 20 + 15
fn test_mixed_arithmetic(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ============================================================
// Integer Overflow Tests
// ============================================================

#[test]
fn test_integer_overflow_addition() {
    let expr = "9223372036854775807 + 1"; // i64::MAX + 1
    let result = compile_and_execute(expr);
    assert!(
        result.is_err(),
        "Addition overflow should produce an error, got: {:?}",
        result
    );
}

#[test]
fn test_integer_overflow_subtraction() {
    let expr = "-9223372036854775808 - 1"; // i64::MIN - 1
    let result = compile_and_execute(expr);
    assert!(
        result.is_err(),
        "Subtraction overflow should produce an error, got: {:?}",
        result
    );
}

#[test]
fn test_integer_overflow_multiplication() {
    let expr = "9223372036854775807 * 2"; // i64::MAX * 2
    let result = compile_and_execute(expr);
    assert!(
        result.is_err(),
        "Multiplication overflow should produce an error, got: {:?}",
        result
    );
}

#[test]
fn test_special_division_overflow() {
    let expr = "-9223372036854775808 / -1"; // i64::MIN / -1
    let result = compile_and_execute(expr);
    assert!(
        result.is_err(),
        "Special case division overflow (i64::MIN / -1) should produce an error, got: {:?}",
        result
    );
}

#[test]
fn test_special_modulo_overflow() {
    let expr = "-9223372036854775808 % -1"; // i64::MIN % -1
    let result = compile_and_execute(expr);
    assert!(
        result.is_err(),
        "Special case modulo overflow (i64::MIN % -1) should produce an error, got: {:?}",
        result
    );
}

#[test]
fn test_safe_arithmetic_at_boundaries() {
    // These operations should work without overflow
    let result =
        compile_and_execute("9223372036854775807 - 1").expect("i64::MAX - 1 should not overflow");
    assert_eq!(result, 9223372036854775806);

    let result =
        compile_and_execute("-9223372036854775808 + 1").expect("i64::MIN + 1 should not overflow");
    assert_eq!(result, -9223372036854775807);

    let result = compile_and_execute("4611686018427387903 * 2")
        .expect("(i64::MAX / 2) * 2 should not overflow");
    assert_eq!(result, 9223372036854775806);
}

#[test]
fn test_negative_overflow_addition() {
    let expr = "-9223372036854775808 + -1"; // i64::MIN + -1
    let result = compile_and_execute(expr);
    assert!(
        result.is_err(),
        "Addition resulting in negative overflow should produce an error, got: {:?}",
        result
    );
}

#[test]
fn test_positive_overflow_subtraction() {
    let expr = "9223372036854775807 - -1"; // i64::MAX - (-1)
    let result = compile_and_execute(expr);
    assert!(
        result.is_err(),
        "Subtraction resulting in positive overflow should produce an error, got: {:?}",
        result
    );
}

// ============================================================
// Comparison Tests
// ============================================================

#[rstest]
#[case("5 == 5", 1)]
#[case("5 == 10", 0)]
#[case("10 == 5", 0)]
#[case("0 == 0", 1)]
#[case("-5 == -5", 1)]
fn test_equality(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("5 != 5", 0)]
#[case("5 != 10", 1)]
#[case("10 != 5", 1)]
#[case("0 != 0", 0)]
fn test_not_equals(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("10 > 5", 1)]
#[case("5 > 10", 0)]
#[case("5 > 5", 0)]
#[case("0 > -5", 1)]
#[case("-5 > 0", 0)]
fn test_greater_than(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("5 < 10", 1)]
#[case("10 < 5", 0)]
#[case("5 < 5", 0)]
#[case("-5 < 0", 1)]
#[case("0 < -5", 0)]
fn test_less_than(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("10 >= 5", 1)]
#[case("5 >= 10", 0)]
#[case("5 >= 5", 1)]
#[case("0 >= -5", 1)]
fn test_greater_or_equal(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("5 <= 10", 1)]
#[case("10 <= 5", 0)]
#[case("5 <= 5", 1)]
#[case("-5 <= 0", 1)]
fn test_less_or_equal(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ============================================================
// Logical Operator Tests
// ============================================================

#[rstest]
#[case("true && true", 1)]
#[case("true && false", 0)]
#[case("false && true", 0)]
#[case("false && false", 0)]
fn test_logical_and(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("true || true", 1)]
#[case("true || false", 1)]
#[case("false || true", 1)]
#[case("false || false", 0)]
fn test_logical_or(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("!true", 0)]
#[case("!false", 1)]
fn test_logical_not(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("5 > 3 && 10 > 7", 1)]
#[case("5 > 10 && 10 > 7", 0)]
#[case("5 > 3 || 10 < 7", 1)]
#[case("5 < 3 || 10 < 7", 0)]
#[case("!(5 > 10)", 1)]
#[case("!(5 > 3)", 0)]
#[case("5 == 5 && 10 == 10", 1)]
#[case("5 != 10 || 3 == 3", 1)]
fn test_combined_logic_and_comparison(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}
