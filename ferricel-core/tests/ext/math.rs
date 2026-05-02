use crate::common::*;
use rstest::rstest;

// ---------------------------------------------------------------------------
// math.greatest
// ---------------------------------------------------------------------------

// Single-arg cases just check they execute without error
#[rstest]
#[case("math.greatest(-5)")]
#[case("math.greatest(5)")]
#[case("math.greatest(-5.0)")]
#[case("math.greatest(5u)")]
fn test_greatest_single_arg(#[case] expr: &str) {
    compile_and_execute(expr).unwrap_or_else(|e| panic!("'{}' should not error: {}", expr, e));
}

// Equality cases return bool
#[rstest]
// Binary int
#[case("math.greatest(1, 1) == 1")]
#[case("math.greatest(3, -3) == 3")]
#[case("math.greatest(-7, 5) == 5")]
#[case("math.greatest(-1, 0) == 0")]
#[case("math.greatest(-1, -1) == -1")]
// Binary cross-type
#[case("math.greatest(1, 1.0) == 1")]
#[case("math.greatest(1, -2.0) == 1")]
#[case("math.greatest(2, 1u) == 2")]
#[case("math.greatest(1.5, 2) == 2")]
#[case("math.greatest(1.5, -2) == 1.5")]
#[case("math.greatest(2.5, 1u) == 2.5")]
#[case("math.greatest(1u, 2) == 2")]
#[case("math.greatest(1u, -2) == 1u")]
#[case("math.greatest(2u, 2.5) == 2.5")]
// Binary double
#[case("math.greatest(42.0, -0.5) == 42.0")]
#[case("math.greatest(1u, 42u) == 42u")]
// Ternary
#[case("math.greatest(-1, 0, 1) == 1")]
#[case("math.greatest(-1, -1, -1) == -1")]
#[case("math.greatest(1u, 42u, 0u) == 42u")]
#[case("math.greatest(42.0, -0.5, -0.25) == 42.0")]
// List argument
#[case("math.greatest([1u, 42u, 0u]) == 42u")]
// Int extremes
#[case("math.greatest(9223372036854775807, 1) == 9223372036854775807")]
#[case("math.greatest(-9223372036854775808, 1) == 1")]
fn test_greatest_true(#[case] expr: &str) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert!(result, "Expression '{}' should be truthy", expr);
}

// ---------------------------------------------------------------------------
// math.least
// ---------------------------------------------------------------------------

// Single-arg cases just check they execute without error
#[rstest]
#[case("math.least(-5)")]
#[case("math.least(5)")]
#[case("math.least(-5.5)")]
#[case("math.least(5u)")]
fn test_least_single_arg(#[case] expr: &str) {
    compile_and_execute(expr).unwrap_or_else(|e| panic!("'{}' should not error: {}", expr, e));
}

// Equality cases return bool
#[rstest]
// Binary int
#[case("math.least(1, 1) == 1")]
#[case("math.least(-3, 3) == -3")]
#[case("math.least(5, -7) == -7")]
#[case("math.least(-1, 0) == -1")]
#[case("math.least(-1, -1) == -1")]
// Binary cross-type
#[case("math.least(1, 1.0) == 1")]
#[case("math.least(1, -2.0) == -2.0")]
#[case("math.least(2, 1u) == 1u")]
#[case("math.least(1.5, 2) == 1.5")]
#[case("math.least(1.5, -2) == -2")]
#[case("math.least(2.5, 1u) == 1u")]
#[case("math.least(1u, 2) == 1u")]
#[case("math.least(1u, -2) == -2")]
#[case("math.least(2u, 2.5) == 2u")]
// Binary double
#[case("math.least(42.0, -0.5) == -0.5")]
#[case("math.least(1u, 42u) == 1u")]
// Ternary
#[case("math.least(-1, 0, 1) == -1")]
#[case("math.least(-1, -1, -1) == -1")]
#[case("math.least(1u, 42u, 0u) == 0u")]
#[case("math.least(42.0, -0.5, -0.25) == -0.5")]
// List argument
#[case("math.least([1u, 42u, 0u]) == 0u")]
// Int extremes
#[case("math.least(-9223372036854775808, 1) == -9223372036854775808")]
#[case("math.least(9223372036854775807, 1) == 1")]
fn test_least_true(#[case] expr: &str) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert!(result, "Expression '{}' should be truthy", expr);
}

// ---------------------------------------------------------------------------
// Rounding functions
// ---------------------------------------------------------------------------

#[rstest]
#[case("math.ceil(1.2) == 2.0", true)]
#[case("math.ceil(-1.2) == -1.0", true)]
fn test_ceil(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("math.floor(1.2) == 1.0", true)]
#[case("math.floor(-1.2) == -2.0", true)]
fn test_floor(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("math.round(1.2) == 1.0", true)]
#[case("math.round(1.5) == 2.0", true)]
#[case("math.round(-1.5) == -2.0", true)]
#[case("math.round(-1.2) == -1.0", true)]
fn test_round(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[test]
fn test_round_nan() {
    // math.round(NaN) == NaN  (NaN != NaN, so check via isNaN)
    let result = compile_and_execute_bool("math.isNaN(math.round(0.0/0.0))")
        .expect("compile_and_execute_bool");
    assert!(result, "math.round(NaN) should be NaN");
}

#[rstest]
#[case("math.trunc(-1.3) == -1.0", true)]
#[case("math.trunc(1.3) == 1.0", true)]
fn test_trunc(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ---------------------------------------------------------------------------
// math.abs
// ---------------------------------------------------------------------------

#[rstest]
#[case("math.abs(-1) == 1")]
#[case("math.abs(1) == 1")]
#[case("math.abs(-234.5) == 234.5")]
#[case("math.abs(234.5) == 234.5")]
fn test_abs_true(#[case] expr: &str) {
    let result = compile_and_execute_bool(expr).expect("compile_and_execute_bool");
    assert!(result, "Expression '{}' should be truthy", expr);
}

#[test]
fn test_abs_overflow() {
    // math.abs(i64::MIN) should produce an overflow error, which propagates as
    // a CelValue::Error and resolves to a non-truthy runtime error result.
    let err = compile_and_execute("math.abs(-9223372036854775808)");
    assert!(err.is_err(), "math.abs(i64::MIN) should return an error");
    let msg = format!("{:?}", err.unwrap_err());
    assert!(
        msg.contains("overflow"),
        "Expected overflow error, got: {}",
        msg
    );
}

// ---------------------------------------------------------------------------
// math.sign
// ---------------------------------------------------------------------------

#[rstest]
#[case("math.sign(-42) == -1")]
#[case("math.sign(0) == 0")]
#[case("math.sign(42) == 1")]
#[case("math.sign(0u) == 0u")]
#[case("math.sign(42u) == 1u")]
#[case("math.sign(-0.3) == -1.0")]
#[case("math.sign(0.0) == 0.0")]
#[case("math.sign(0.3) == 1.0")]
#[case("math.sign(1.0/0.0) == 1.0")] // +Inf -> 1.0
#[case("math.sign(-1.0/0.0) == -1.0")] // -Inf -> -1.0
fn test_sign_true(#[case] expr: &str) {
    let result = compile_and_execute_bool(expr).expect("compile_and_execute_bool");
    assert!(result, "Expression '{}' should be truthy", expr);
}

#[test]
fn test_sign_nan() {
    let result = compile_and_execute_bool("math.isNaN(math.sign(0.0/0.0))")
        .expect("compile_and_execute_bool");
    assert!(result, "math.sign(NaN) should be NaN");
}

// ---------------------------------------------------------------------------
// Float predicates
// ---------------------------------------------------------------------------

#[rstest]
#[case("math.isNaN(0.0/0.0)")]
#[case("!math.isNaN(1.0/0.0)")]
#[case("math.isFinite(1.0/1.5)")]
#[case("!math.isFinite(1.0/0.0)")]
#[case("!math.isFinite(0.0/0.0)")]
#[case("math.isInf(1.0/0.0)")]
#[case("!math.isInf(0.0/0.0)")]
#[case("!math.isInf(1.2)")]
fn test_float_predicates(#[case] expr: &str) {
    let result = compile_and_execute_bool(expr).expect("compile_and_execute_bool");
    assert!(result, "Expression '{}' should be truthy", expr);
}

// ---------------------------------------------------------------------------
// Bitwise operations
// ---------------------------------------------------------------------------

#[rstest]
// Signed bitwise ops
#[case("math.bitAnd(1, 2) == 0")]
#[case("math.bitAnd(1, -1) == 1")]
#[case("math.bitAnd(1, 3) == 1")]
#[case("math.bitOr(1, 2) == 3")]
#[case("math.bitXor(1, 3) == 2")]
#[case("math.bitXor(3, 5) == 6")]
#[case("math.bitNot(1) == -2")]
#[case("math.bitNot(0) == -1")]
#[case("math.bitNot(-1) == 0")]
// Shifts (signed)
#[case("math.bitShiftLeft(1, 2) == 4")]
#[case("math.bitShiftLeft(1, 200) == 0")]
#[case("math.bitShiftLeft(-1, 200) == 0")]
#[case("math.bitShiftRight(1024, 2) == 256")]
#[case("math.bitShiftRight(1024, 64) == 0")]
#[case("math.bitShiftRight(-1024, 3) == 2305843009213693824")] // logical shift
#[case("math.bitShiftRight(-1024, 64) == 0")]
// Unsigned bitwise ops
#[case("math.bitAnd(1u, 2u) == 0u")]
#[case("math.bitAnd(1u, 3u) == 1u")]
#[case("math.bitOr(1u, 2u) == 3u")]
#[case("math.bitXor(1u, 3u) == 2u")]
#[case("math.bitXor(3u, 5u) == 6u")]
#[case("math.bitNot(1u) == 18446744073709551614u")]
#[case("math.bitNot(0u) == 18446744073709551615u")]
// Shifts (unsigned)
#[case("math.bitShiftLeft(1u, 2) == 4u")]
#[case("math.bitShiftLeft(1u, 200) == 0u")]
#[case("math.bitShiftRight(1024u, 2) == 256u")]
#[case("math.bitShiftRight(1024u, 64) == 0u")]
fn test_bitwise_true(#[case] expr: &str) {
    let result = compile_and_execute_bool(expr).expect("compile_and_execute_bool");
    assert!(result, "Expression '{}' should be truthy", expr);
}

// ---------------------------------------------------------------------------
// Bitwise error cases
// ---------------------------------------------------------------------------

#[test]
fn test_bit_shift_left_negative_offset() {
    let err = compile_and_execute("math.bitShiftLeft(1, -2)");
    assert!(err.is_err(), "negative offset should produce an error");
    let msg = format!("{:?}", err.unwrap_err());
    assert!(
        msg.contains("negative offset"),
        "Expected 'negative offset' error, got: {}",
        msg
    );
}

#[test]
fn test_bit_shift_right_negative_offset() {
    let err = compile_and_execute("math.bitShiftRight(-1024, -3)");
    assert!(err.is_err(), "negative offset should produce an error");
    let msg = format!("{:?}", err.unwrap_err());
    assert!(
        msg.contains("negative offset"),
        "Expected 'negative offset' error, got: {}",
        msg
    );
}

// ---------------------------------------------------------------------------
// math.sqrt
// ---------------------------------------------------------------------------

#[rstest]
#[case("math.sqrt(49.0) == 7.0", true)]
#[case("math.sqrt(0) == 0.0", true)]
#[case("math.sqrt(1) == 1.0", true)]
#[case("math.sqrt(25u) == 5.0", true)]
fn test_sqrt(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("compile_and_execute_bool");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Cases where floating-point precision makes exact equality fragile — check via abs
#[rstest]
#[case("math.sqrt(82)", 9.055385138137417_f64)]
#[case("math.sqrt(985.25)", 31.388692231439016_f64)]
fn test_sqrt_precision(#[case] expr: &str, #[case] expected: f64) {
    let result = compile_and_execute_double(expr).expect("compile_and_execute_double");
    let tolerance = expected.abs() * 1e-9 + 1e-12;
    assert!(
        (result - expected).abs() <= tolerance,
        "Expression '{}': got {}, expected {}",
        expr,
        result,
        expected
    );
}

#[test]
fn test_sqrt_negative_is_nan() {
    let result = compile_and_execute_bool("math.isNaN(math.sqrt(-15.34))")
        .expect("compile_and_execute_bool");
    assert!(result, "math.sqrt(-15.34) should be NaN");
}

// ---------------------------------------------------------------------------
// Type-mismatch runtime errors (ported from TestMathRuntimeErrors)
// ---------------------------------------------------------------------------

#[rstest]
#[case("math.bitOr(dyn(1.2), 1)", "no such overload")]
#[case("math.bitAnd(2u, dyn(''))", "no such overload")]
#[case("math.bitXor(dyn([]), dyn([1]))", "no such overload")]
#[case("math.bitNot(dyn([1]))", "no such overload")]
#[case("math.bitShiftLeft(dyn([1]), 1)", "no such overload")]
#[case("math.bitShiftRight(dyn({}), 1)", "no such overload")]
#[case("math.isInf(dyn(1u))", "no such overload")]
#[case("math.isFinite(dyn(1u))", "no such overload")]
#[case("math.isNaN(dyn(1u))", "no such overload")]
#[case("math.sign(dyn(''))", "no such overload")]
#[case("math.abs(dyn(''))", "no such overload")]
#[case("math.ceil(dyn(''))", "no such overload")]
#[case("math.floor(dyn(''))", "no such overload")]
#[case("math.round(dyn(1))", "no such overload")]
#[case("math.trunc(dyn(1u))", "no such overload")]
#[case("math.sqrt(dyn(''))", "no such overload")]
fn test_runtime_type_errors(#[case] expr: &str, #[case] expected_fragment: &str) {
    let err = compile_and_execute(expr);
    assert!(
        err.is_err(),
        "Expression '{}' should produce a runtime error",
        expr
    );
    let msg = format!("{:?}", err.unwrap_err());
    assert!(
        msg.contains(expected_fragment),
        "Expression '{}': expected error containing '{}', got: {}",
        expr,
        expected_fragment,
        msg
    );
}
