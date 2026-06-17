//! Integration tests for the Kubernetes CEL quantity library.

use rstest::rstest;

use crate::common::*;

// ── isQuantity(string) ───────────────────────────────────────────────────────

#[rstest]
#[case(r#"isQuantity("0")"#, true)]
#[case(r#"isQuantity("1")"#, true)]
#[case(r#"isQuantity("1k")"#, true)]
#[case(r#"isQuantity("1M")"#, true)]
#[case(r#"isQuantity("1G")"#, true)]
#[case(r#"isQuantity("1T")"#, true)]
#[case(r#"isQuantity("1P")"#, true)]
#[case(r#"isQuantity("1E")"#, true)]
#[case(r#"isQuantity("1Ki")"#, true)]
#[case(r#"isQuantity("1Mi")"#, true)]
#[case(r#"isQuantity("1Gi")"#, true)]
#[case(r#"isQuantity("1Ti")"#, true)]
#[case(r#"isQuantity("1Pi")"#, true)]
#[case(r#"isQuantity("1Ei")"#, true)]
#[case(r#"isQuantity("100m")"#, true)]
#[case(r#"isQuantity("1.5")"#, true)]
#[case(r#"isQuantity("1e3")"#, true)]
#[case(r#"isQuantity("-1")"#, true)]
#[case(r#"isQuantity("+1")"#, true)]
#[case(r#"isQuantity("")"#, false)]
#[case(r#"isQuantity("abc")"#, false)]
fn test_is_quantity(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── quantity(string) — successful parse ──────────────────────────────────────

#[rstest]
#[case(r#"string(quantity("100m"))"#, "100m")]
#[case(r#"string(quantity("1k"))"#, "1k")]
#[case(r#"string(quantity("1Ki"))"#, "1Ki")]
fn test_quantity_parse(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {:?}",
        expr, expected
    );
}

// ── quantity(string) — error on invalid ──────────────────────────────────────

#[test]
fn test_quantity_parse_invalid_returns_error() {
    let result = compile_and_execute(r#"quantity("abc").sign()"#);
    assert!(
        result.is_err(),
        "Expected error for quantity(\"abc\"), got {:?}",
        result
    );
}

// ── sign() ───────────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"quantity("1").sign()"#, 1)]
#[case(r#"quantity("0").sign()"#, 0)]
#[case(r#"quantity("-1").sign()"#, -1)]
#[case(r#"quantity("100m").sign()"#, 1)]
#[case(r#"quantity("-100m").sign()"#, -1)]
fn test_sign(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── isInteger() ──────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"quantity("1").isInteger()"#, true)]
#[case(r#"quantity("1k").isInteger()"#, true)]
#[case(r#"quantity("1Ki").isInteger()"#, true)]
#[case(r#"quantity("100m").isInteger()"#, false)]
#[case(r#"quantity("1.5").isInteger()"#, false)]
fn test_is_integer(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── asInteger() ──────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"quantity("1").asInteger()"#, 1)]
#[case(r#"quantity("1k").asInteger()"#, 1000)]
#[case(r#"quantity("2Ki").asInteger()"#, 2048)]
fn test_as_integer_ok(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

#[rstest]
#[case(r#"quantity("100m").asInteger()"#)]
#[case(r#"quantity("1.5").asInteger()"#)]
#[case(r#"quantity("9999999999999999999999999999999999999G").asInteger()"#)]
#[case(r#"quantity("-9999999999999999999999999999999999999G").asInteger()"#)]
fn test_as_integer_error(#[case] expr: &str) {
    let result = compile_and_execute(expr);
    assert!(
        result.is_err(),
        "Expected error for '{}', got {:?}",
        expr,
        result
    );
}

// ── asApproximateFloat() ─────────────────────────────────────────────────────

#[rstest]
#[case(r#"quantity("1").asApproximateFloat()"#, 1.0f64)]
#[case(r#"quantity("1k").asApproximateFloat()"#, 1000.0f64)]
#[case(r#"quantity("100m").asApproximateFloat()"#, 0.1f64)]
fn test_as_approx_float(#[case] expr: &str, #[case] expected: f64) {
    let result = compile_and_execute_double(expr).expect("Failed to compile and execute");
    assert!(
        (result - expected).abs() < 1e-9,
        "Expression '{}': expected {}, got {}",
        expr,
        expected,
        result
    );
}

// ── add(<Q>) ─────────────────────────────────────────────────────────────────

#[rstest]
#[case(
    r#"quantity("50k").add(quantity("50k")).compareTo(quantity("100k")) == 0"#,
    true
)]
#[case(
    r#"quantity("200M").add(quantity("100k")).compareTo(quantity("200100k")) == 0"#,
    true
)]
fn test_add_quantities(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── add(int) ─────────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"quantity("50k").add(20).compareTo(quantity("50020")) == 0"#, true)]
fn test_add_int(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── sub(<Q>) ─────────────────────────────────────────────────────────────────

#[rstest]
#[case(
    r#"quantity("100k").sub(quantity("50k")).compareTo(quantity("50k")) == 0"#,
    true
)]
fn test_sub_quantities(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── sub(int) ─────────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"quantity("50k").sub(20).compareTo(quantity("49980")) == 0"#, true)]
#[case(
    r#"quantity("50k").sub(-50000).compareTo(quantity("100k")) == 0"#,
    true
)]
fn test_sub_int(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── chained arithmetic ───────────────────────────────────────────────────────

#[test]
fn test_chained_add_sub() {
    let result = compile_and_execute_bool(
        r#"quantity("50k").add(20).sub(quantity("100k")).sub(-50000).compareTo(quantity("20")) == 0"#,
    )
    .expect("Failed to compile and execute");
    assert!(result, "chained arithmetic should equal 20");
}

// ── isLessThan() ─────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"quantity("100m").isLessThan(quantity("200m"))"#, true)]
#[case(r#"quantity("200m").isLessThan(quantity("100m"))"#, false)]
#[case(r#"quantity("100m").isLessThan(quantity("100m"))"#, false)]
#[case(r#"quantity("1k").isLessThan(quantity("1M"))"#, true)]
fn test_is_less_than(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── isGreaterThan() ──────────────────────────────────────────────────────────

#[rstest]
#[case(r#"quantity("200m").isGreaterThan(quantity("100m"))"#, true)]
#[case(r#"quantity("100m").isGreaterThan(quantity("200m"))"#, false)]
#[case(r#"quantity("100m").isGreaterThan(quantity("100m"))"#, false)]
fn test_is_greater_than(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── compareTo() ──────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"quantity("100m").compareTo(quantity("200m"))"#, -1)]
#[case(r#"quantity("200m").compareTo(quantity("100m"))"#, 1)]
#[case(r#"quantity("100m").compareTo(quantity("100m"))"#, 0)]
#[case(r#"quantity("200M").compareTo(quantity("0.2G"))"#, 0)]
fn test_compare_to(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── equality (==) ────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"quantity("1k") == quantity("1k")"#, true)]
#[case(r#"quantity("200M") == quantity("0.2G")"#, true)]
#[case(r#"quantity("1Ki") == quantity("1024")"#, true)]
#[case(r#"quantity("1k") == quantity("1000")"#, true)]
#[case(r#"quantity("1k") == quantity("2k")"#, false)]
fn test_equality(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── cross-type dispatch errors ───────────────────────────────────────────────

#[rstest]
#[case(r#"quantity("1k").isLessThan(semver("1.0.0"))"#)]
#[case(r#"quantity("1k").isGreaterThan(semver("1.0.0"))"#)]
#[case(r#"quantity("1k").compareTo(semver("1.0.0"))"#)]
fn test_quantity_cross_type_dispatch_error(#[case] expr: &str) {
    let result = compile_and_execute(expr);
    assert!(
        result.is_err(),
        "Expected error for '{}', got {:?}",
        expr,
        result
    );
}

// ── overflow ─────────────────────────────────────────────────────────────────

#[rstest]
// isQuantity accepts overflow strings as valid
#[case(r#"isQuantity("9999999999999999999999999999999999999G")"#, true)]
#[case(r#"isQuantity("-9999999999999999999999999999999999999G")"#, true)]
// isInteger — overflow is never an integer
#[case(
    r#"quantity("9999999999999999999999999999999999999G").isInteger()"#,
    false
)]
// asApproximateFloat comparisons
#[case(
    r#"quantity("9999999999999999999999999999999999999G").asApproximateFloat() > 1e300"#,
    true
)]
#[case(
    r#"quantity("-9999999999999999999999999999999999999G").asApproximateFloat() < -1e300"#,
    true
)]
// comparisons
#[case(
    r#"quantity("9999999999999999999999999999999999999G").isGreaterThan(quantity("1k"))"#,
    true
)]
#[case(
    r#"quantity("1k").isLessThan(quantity("9999999999999999999999999999999999999G"))"#,
    true
)]
#[case(
    r#"quantity("-9999999999999999999999999999999999999G").isLessThan(quantity("1k"))"#,
    true
)]
// equality
#[case(r#"quantity("9999999999999999999999999999999999999G") == quantity("9999999999999999999999999999999999999G")"#, true)]
#[case(r#"quantity("9999999999999999999999999999999999999G") == quantity("-9999999999999999999999999999999999999G")"#, false)]
#[case(
    r#"quantity("9999999999999999999999999999999999999G") == quantity("1k")"#,
    false
)]
fn test_overflow(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// overflow sign() and arithmetic sign() — these return i64
#[rstest]
#[case(r#"quantity("9999999999999999999999999999999999999G").sign()"#, 1)]
#[case(r#"quantity("-9999999999999999999999999999999999999G").sign()"#, -1)]
#[case(
    r#"quantity("9999999999999999999999999999999999999G").add(quantity("1k")).sign()"#,
    1
)]
#[case(
    r#"quantity("9999999999999999999999999999999999999G").sub(quantity("1k")).sign()"#,
    1
)]
#[case(
    r#"quantity("9999999999999999999999999999999999999G").compareTo(quantity("1k"))"#,
    1
)]
#[case(r#"quantity("-9999999999999999999999999999999999999G").compareTo(quantity("1k"))"#, -1)]
fn test_overflow_int(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}
