// Integration tests for the ferricel-core compiler.
// These tests compile CEL expressions to WASM and execute them to verify correctness.
//
// Shared test helpers live in `common/mod.rs`. To add a new test file, create
// `tests/foo_tests.rs`, add `mod common; use common::*;` at the top, and move
// the relevant test functions there.

mod common;
use common::*;

use ferricel_core::compiler::{compile_cel_to_wasm, CompilerOptions};

#[test]
fn test_compile_cel_to_wasm_returns_valid_bytes() {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger,
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm("42", compiler_options).expect("Failed to compile");
    assert!(!wasm_bytes.is_empty(), "WASM bytes should not be empty");

    // WASM files start with magic number: 0x00 0x61 0x73 0x6D (\\0asm)
    assert_eq!(
        &wasm_bytes[0..4],
        &[0x00, 0x61, 0x73, 0x6D],
        "Should have WASM magic number"
    );
}

#[test]
fn test_invalid_cel_expression() {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger,
        extensions: vec![],
    };
    let result = compile_cel_to_wasm("1 + + 2", compiler_options);
    assert!(
        result.is_err(),
        "Invalid CEL expression should return error"
    );
}

// ===== Double Literal Tests =====
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

// ===== Double Arithmetic Tests =====
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

// ===== Double Comparison Tests =====
#[rstest]
#[case("3.14 == 3.14", 1)]
#[case("3.14 == 2.71", 0)]
#[case("0.0 == 0.0", 1)]
fn test_double_equality(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("5.0 > 3.0", 1)]
#[case("3.0 > 5.0", 0)]
#[case("5.0 > 5.0", 0)]
#[case("5.0 >= 5.0", 1)]
#[case("5.0 >= 3.0", 1)]
#[case("3.0 >= 5.0", 0)]
fn test_double_greater_than(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case("3.0 < 5.0", 1)]
#[case("5.0 < 3.0", 0)]
#[case("5.0 < 5.0", 0)]
#[case("5.0 <= 5.0", 1)]
#[case("3.0 <= 5.0", 1)]
#[case("5.0 <= 3.0", 0)]
fn test_double_less_than(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ===== Type Safety Tests (No Auto-Coercion) =====
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

// ===== Comparison Tests =====
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

// ===== Logical Operator Tests =====
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

// ===== Combined Logic and Comparison Tests =====
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
// JSON Output Tests
// ============================================================

#[test]
fn test_json_output_integer() {
    // Test that integers are serialized as raw JSON numbers
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm("42", compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(
        json_result, "42",
        "Integer should be serialized as raw JSON number"
    );
}

#[test]
fn test_json_output_boolean_true() {
    // Test that true is serialized as raw JSON boolean
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm("5 > 3", compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(
        json_result, "true",
        "Boolean true should be serialized as 'true'"
    );
}

#[test]
fn test_json_output_boolean_false() {
    // Test that false is serialized as raw JSON boolean
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm("5 < 3", compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(
        json_result, "false",
        "Boolean false should be serialized as 'false'"
    );
}

#[test]
fn test_json_output_negative_integer() {
    // Test that negative integers are properly serialized
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm("-123", compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(
        json_result, "-123",
        "Negative integer should be serialized correctly"
    );
}

#[test]
fn test_json_output_arithmetic_result() {
    // Test that arithmetic results are serialized correctly
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes =
        compile_cel_to_wasm("10 + 20 * 2", compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(
        json_result, "50",
        "Arithmetic result should be serialized correctly"
    );
}

// ========================================
// List Literal Tests
// ========================================

#[rstest]
#[case::empty("[]", "[]")]
#[case::single_element("[42]", "[42]")]
#[case::multiple_integers("[1, 2, 3]", "[1,2,3]")]
#[case::with_expressions("[1 + 1, 2 * 3, 10 - 5]", "[2,6,5]")]
#[case::mixed_types("[1, true, 3, false]", "[1,true,3,false]")]
#[case::with_comparisons("[5 > 3, 2 < 1, 10 == 10]", "[true,false,true]")]
#[case::concatenation("[1, 2] + [3, 4]", "[1,2,3,4]")]
#[case::concatenation_empty("[] + []", "[]")]
#[case::concatenation_with_empty("[1, 2, 3] + []", "[1,2,3]")]
fn test_list_literals(#[case] expr: &str, #[case] expected: &str) {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm(expr, compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(json_result, expected);
}

// ========================================
// all() Macro Tests
// ========================================

#[rstest]
#[case::all_true("[1, 2, 3].all(x, x > 0)", "true")]
#[case::some_false("[1, -2, 3].all(x, x > 0)", "false")]
#[case::empty_list("[].all(x, x > 0)", "true")]
#[case::complex_predicate("[10, 20, 30].all(x, x >= 10 && x <= 30)", "true")]
#[case::equality("[5, 5, 5].all(x, x == 5)", "true")]
#[case::single_false("[1, 2, 3, 0].all(x, x > 0)", "false")]
#[case::with_expressions("[1+1, 2*3, 10-5].all(x, x > 1)", "true")]
fn test_all_macro(#[case] expr: &str, #[case] expected: &str) {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm(expr, compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(json_result, expected);
}

// ========================================
// exists() Macro Tests
// ========================================

#[rstest]
#[case::one_true("[1, 2, 3].exists(x, x > 2)", "true")]
#[case::all_false("[1, 2, 3].exists(x, x > 10)", "false")]
#[case::empty_list("[].exists(x, x > 0)", "false")]
#[case::all_true("[5, 10, 15].exists(x, x > 0)", "true")]
#[case::complex_predicate("[1, 5, 10].exists(x, x >= 5 && x <= 10)", "true")]
#[case::first_element_true("[10, 1, 2].exists(x, x > 5)", "true")]
#[case::last_element_true("[1, 2, 10].exists(x, x > 5)", "true")]
fn test_exists_macro(#[case] expr: &str, #[case] expected: &str) {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm(expr, compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(json_result, expected);
}

// ========================================
// exists_one() Macro Tests
// ========================================

#[rstest]
#[case::exactly_one("[1, 5, 3].exists_one(x, x > 4)", "true")]
#[case::none("[1, 2, 3].exists_one(x, x > 10)", "false")]
#[case::multiple("[5, 10, 15].exists_one(x, x > 4)", "false")]
#[case::empty_list("[].exists_one(x, x > 0)", "false")]
#[case::first_element_only("[10, 1, 2].exists_one(x, x > 5)", "true")]
#[case::last_element_only("[1, 2, 10].exists_one(x, x > 5)", "true")]
#[case::two_elements("[10, 20, 1].exists_one(x, x > 5)", "false")]
fn test_exists_one_macro(#[case] expr: &str, #[case] expected: &str) {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm(expr, compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(json_result, expected);
}

// ========================================
// filter() Macro Tests
// ========================================

#[rstest]
#[case::basic("[1, 2, 3, 4, 5].filter(x, x > 2)", "[3,4,5]")]
#[case::none_match("[1, 2, 3].filter(x, x > 10)", "[]")]
#[case::all_match("[1, 2, 3].filter(x, x > 0)", "[1,2,3]")]
#[case::empty_list("[].filter(x, x > 0)", "[]")]
#[case::even_numbers("[1, 2, 3, 4, 5, 6].filter(x, x % 2 == 0)", "[2,4,6]")]
#[case::complex_predicate("[1, 5, 10, 15, 20].filter(x, x >= 5 && x <= 15)", "[5,10,15]")]
#[case::first_element_only("[10, 1, 2, 3].filter(x, x > 5)", "[10]")]
#[case::last_element_only("[1, 2, 3, 10].filter(x, x > 5)", "[10]")]
fn test_filter_macro(#[case] expr: &str, #[case] expected: &str) {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm(expr, compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(json_result, expected);
}

// ========================================
// map() Macro Tests
// ========================================

#[rstest]
#[case::basic("[1, 2, 3].map(x, x * 2)", "[2,4,6]")]
#[case::empty_list("[].map(x, x * 2)", "[]")]
#[case::identity("[1, 2, 3].map(x, x)", "[1,2,3]")]
#[case::addition("[1, 2, 3].map(x, x + 10)", "[11,12,13]")]
#[case::square("[1, 2, 3, 4].map(x, x * x)", "[1,4,9,16]")]
#[case::type_change("[1, 2, 3].map(x, x > 1)", "[false,true,true]")]
#[case::division("[10, 20, 30].map(x, x / 10)", "[1,2,3]")]
#[case::complex_expression("[1, 2, 3].map(x, (x * 2) + 1)", "[3,5,7]")]
#[case::single_element("[5].map(x, x * 2)", "[10]")]
#[case::negative_numbers("[-1, -2, -3].map(x, x * -1)", "[1,2,3]")]
#[case::modulo("[10, 11, 12].map(x, x % 3)", "[1,2,0]")]
fn test_map_macro(#[case] expr: &str, #[case] expected: &str) {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm(expr, compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(json_result, expected);
}

// ========================================
// Variable Access Tests (PR #4)
// ========================================

#[test]
fn test_input_variable_positive() {
    // Test accessing input variable with a positive integer
    let result =
        compile_and_execute_with_vars("input", Some("42"), None).expect("Failed to execute");
    assert_eq!(result, 42, "input should return 42");
}

#[test]
fn test_input_variable_negative() {
    // Test accessing input variable with a negative integer
    let result =
        compile_and_execute_with_vars("input", Some("-10"), None).expect("Failed to execute");
    assert_eq!(result, -10, "input should return -10");
}

#[test]
fn test_input_variable_zero() {
    // Test accessing input variable with zero
    let result =
        compile_and_execute_with_vars("input", Some("0"), None).expect("Failed to execute");
    assert_eq!(result, 0, "input should return 0");
}

#[test]
fn test_data_variable_positive() {
    // Test accessing data variable with a positive integer
    let result =
        compile_and_execute_with_vars("data", None, Some("100")).expect("Failed to execute");
    assert_eq!(result, 100, "data should return 100");
}

#[test]
fn test_data_variable_negative() {
    // Test accessing data variable with a negative integer
    let result =
        compile_and_execute_with_vars("data", None, Some("-50")).expect("Failed to execute");
    assert_eq!(result, -50, "data should return -50");
}

#[test]
fn test_input_and_data_addition() {
    // Test using both input and data in an expression
    let result = compile_and_execute_with_vars("input + data", Some("10"), Some("20"))
        .expect("Failed to execute");
    assert_eq!(result, 30, "input + data should return 30");
}

#[test]
fn test_input_and_data_multiplication() {
    // Test multiplication with input and data
    let result = compile_and_execute_with_vars("input * data", Some("5"), Some("7"))
        .expect("Failed to execute");
    assert_eq!(result, 35, "input * data should return 35");
}

#[test]
fn test_input_in_complex_expression() {
    // Test input in a more complex expression
    let result = compile_and_execute_with_vars("input * 2 + 10", Some("5"), None)
        .expect("Failed to execute");
    assert_eq!(result, 20, "input * 2 + 10 should return 20");
}

#[test]
fn test_data_in_complex_expression() {
    // Test data in a more complex expression
    let result = compile_and_execute_with_vars("(data - 5) * 3", None, Some("10"))
        .expect("Failed to execute");
    assert_eq!(result, 15, "(data - 5) * 3 should return 15");
}

#[test]
fn test_input_variable_i64_max() {
    // Test with i64::MAX
    let max = i64::MAX;
    let input_json = format!("{}", max);
    let result =
        compile_and_execute_with_vars("input", Some(&input_json), None).expect("Failed to execute");
    assert_eq!(result, max, "input should return i64::MAX");
}

#[test]
fn test_input_variable_i64_min() {
    // Test with i64::MIN
    let min = i64::MIN;
    let input_json = format!("{}", min);
    let result =
        compile_and_execute_with_vars("input", Some(&input_json), None).expect("Failed to execute");
    assert_eq!(result, min, "input should return i64::MIN");
}

// ========================================
// Field Access Tests
// ========================================

#[test]
fn test_simple_field_access() {
    // Test accessing a field from input object
    let input_json = r#"{"age": 42}"#;
    let result = compile_and_execute_with_vars("input.age", Some(input_json), None)
        .expect("Failed to execute");
    assert_eq!(result, 42, "input.age should return 42");
}

#[test]
fn test_nested_field_access() {
    // Test accessing nested fields
    let input_json = r#"{"user": {"age": 30}}"#;
    let result = compile_and_execute_with_vars("input.user.age", Some(input_json), None)
        .expect("Failed to execute");
    assert_eq!(result, 30, "input.user.age should return 30");
}

#[test]
fn test_field_access_with_data() {
    // Test field access on data variable
    let data_json = r#"{"count": 100}"#;
    let result = compile_and_execute_with_vars("data.count", None, Some(data_json))
        .expect("Failed to execute");
    assert_eq!(result, 100, "data.count should return 100");
}

#[test]
fn test_field_access_in_expression() {
    // Test using field access in arithmetic
    let input_json = r#"{"x": 10}"#;
    let result = compile_and_execute_with_vars("input.x * 2 + 5", Some(input_json), None)
        .expect("Failed to execute");
    assert_eq!(result, 25, "input.x * 2 + 5 should return 25");
}

#[test]
fn test_multiple_field_access() {
    // Test accessing fields from both input and data
    let input_json = r#"{"a": 10}"#;
    let data_json = r#"{"b": 20}"#;
    let result =
        compile_and_execute_with_vars("input.a + data.b", Some(input_json), Some(data_json))
            .expect("Failed to execute");
    assert_eq!(result, 30, "input.a + data.b should return 30");
}

#[test]
fn test_deeply_nested_field_access() {
    // Test accessing deeply nested fields
    let input_json = r#"{"level1": {"level2": {"level3": {"value": 99}}}}"#;
    let result =
        compile_and_execute_with_vars("input.level1.level2.level3.value", Some(input_json), None)
            .expect("Failed to execute");
    assert_eq!(result, 99, "deeply nested field should return 99");
}

// ============================================================================
// HAS MACRO TESTS
// ============================================================================

#[rstest]
#[case(r#"{"name": "Alice", "age": 30}"#, "has(input.name)", 1)]
#[case(r#"{"name": "Alice", "age": 30}"#, "has(input.age)", 1)]
#[case(r#"{"name": "Alice"}"#, "has(input.age)", 0)]
#[case(r#"{"name": "Alice"}"#, "has(input.email)", 0)]
#[case(r#"{}"#, "has(input.anything)", 0)]
fn test_has_macro_basic(#[case] input_json: &str, #[case] expr: &str, #[case] expected: i64) {
    let result =
        compile_and_execute_with_vars(expr, Some(input_json), None).expect("Failed to execute");
    assert_eq!(
        result, expected,
        "Expression '{}' with input {} should evaluate to {}",
        expr, input_json, expected
    );
}

#[rstest]
#[case(r#"{"user": {"name": "Bob"}}"#, "has(input.user.name)", 1)]
#[case(r#"{"user": {"name": "Bob"}}"#, "has(input.user.age)", 0)]
#[case(r#"{"user": {}}"#, "has(input.user.name)", 0)]
#[case(r#"{"a": {"b": {"c": 42}}}"#, "has(input.a.b.c)", 1)]
#[case(r#"{"a": {"b": {}}}"#, "has(input.a.b.c)", 0)]
fn test_has_macro_nested(#[case] input_json: &str, #[case] expr: &str, #[case] expected: i64) {
    let result =
        compile_and_execute_with_vars(expr, Some(input_json), None).expect("Failed to execute");
    assert_eq!(
        result, expected,
        "Expression '{}' with input {} should evaluate to {}",
        expr, input_json, expected
    );
}

#[test]
fn test_has_macro_with_data_variable() {
    let data_json = r#"{"config": {"enabled": true}}"#;
    let result = compile_and_execute_with_vars("has(data.config)", None, Some(data_json))
        .expect("Failed to execute");
    assert_eq!(result, 1, "has(data.config) should return true");
}

#[test]
fn test_has_macro_with_null_value() {
    // Field exists but value is null - should return true
    let input_json = r#"{"nullable": null}"#;
    let result = compile_and_execute_with_vars("has(input.nullable)", Some(input_json), None)
        .expect("Failed to execute");
    assert_eq!(
        result, 1,
        "has(input.nullable) should return true even when value is null"
    );
}

#[rstest]
#[case(r#"{"age": 25}"#, "has(input.age) && input.age > 18", 1)]
#[case(r#"{"age": 15}"#, "has(input.age) && input.age > 18", 0)]
#[case(r#"{"age": 25}"#, "has(input.age) || has(input.name)", 1)]
#[case(r#"{}"#, "has(input.age) || has(input.name)", 0)]
#[case(r#"{"name": "Alice"}"#, "!has(input.age)", 1)]
#[case(r#"{"age": 25}"#, "!has(input.missing)", 1)]
fn test_has_macro_in_expressions(
    #[case] input_json: &str,
    #[case] expr: &str,
    #[case] expected: i64,
) {
    let result =
        compile_and_execute_with_vars(expr, Some(input_json), None).expect("Failed to execute");
    assert_eq!(
        result, expected,
        "Expression '{}' with input {} should evaluate to {}",
        expr, input_json, expected
    );
}

#[rstest]
#[case(r#"{"a": 1, "b": 2}"#, "has(input.a) && has(input.b)", 1)]
#[case(
    r#"{"a": 1, "b": 2}"#,
    "has(input.a) && has(input.b) && !has(input.c)",
    1
)]
#[case(r#"{"a": 1}"#, "has(input.a) && has(input.b)", 0)]
#[case(
    r#"{"a": 1, "b": 2, "c": 3}"#,
    "has(input.a) && has(input.b) && has(input.c)",
    1
)]
fn test_has_macro_multiple_fields(
    #[case] input_json: &str,
    #[case] expr: &str,
    #[case] expected: i64,
) {
    let result =
        compile_and_execute_with_vars(expr, Some(input_json), None).expect("Failed to execute");
    assert_eq!(
        result, expected,
        "Expression '{}' with input {} should evaluate to {}",
        expr, input_json, expected
    );
}

// ============================================================
// String Tests
// ============================================================

#[rstest]
#[case::basic(r#""hello""#, "hello")]
#[case::empty(r#""""#, "")]
#[case::with_spaces(r#""hello world""#, "hello world")]
#[case::unicode(r#""こんにちは""#, "こんにちは")]
#[case::emoji(r#""hello 👋 world""#, "hello 👋 world")]
#[case::special_chars(r#""!@#$%^&*()""#, "!@#$%^&*()")]
fn test_string_literals(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

#[rstest]
#[case::basic(r#""hello" + " world""#, "hello world")]
#[case::empty_left(r#""" + "test""#, "test")]
#[case::empty_right(r#""test" + """#, "test")]
#[case::both_empty(r#""" + """#, "")]
#[case::unicode(r#""Hello " + "世界""#, "Hello 世界")]
#[case::emoji(r#""Hello " + "👋🌍""#, "Hello 👋🌍")]
#[case::multiple(r#""a" + "b" + "c""#, "abc")]
#[case::with_spaces(r#""hello " + "beautiful " + "world""#, "hello beautiful world")]
fn test_string_concatenation(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

#[rstest]
#[case::basic(r#"size("hello")"#, 5)]
#[case::empty(r#"size("")"#, 0)]
#[case::with_spaces(r#"size("hello world")"#, 11)]
#[case::unicode(r#"size("こんにちは")"#, 5)]
#[case::emoji(r#"size("👋🌍")"#, 2)]
#[case::mixed(r#"size("Hello 世界")"#, 8)]
#[case::concatenation(r#"size("abc" + "def")"#, 6)]
fn test_string_size(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case::basic_true(r#""hello".startsWith("he")"#, 1)]
#[case::basic_false(r#""hello".startsWith("wo")"#, 0)]
#[case::empty_prefix(r#""hello".startsWith("")"#, 1)]
#[case::full_match(r#""hello".startsWith("hello")"#, 1)]
#[case::longer_prefix(r#""hi".startsWith("hello")"#, 0)]
#[case::unicode(r#""こんにちは".startsWith("こん")"#, 1)]
#[case::emoji(r#""👋🌍".startsWith("👋")"#, 1)]
#[case::case_sensitive(r#""Hello".startsWith("hello")"#, 0)]
fn test_string_starts_with(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case::basic_true(r#""hello".endsWith("lo")"#, 1)]
#[case::basic_false(r#""hello".endsWith("he")"#, 0)]
#[case::empty_suffix(r#""hello".endsWith("")"#, 1)]
#[case::full_match(r#""hello".endsWith("hello")"#, 1)]
#[case::longer_suffix(r#""hi".endsWith("hello")"#, 0)]
#[case::unicode(r#""こんにちは".endsWith("ちは")"#, 1)]
#[case::emoji(r#""👋🌍".endsWith("🌍")"#, 1)]
#[case::case_sensitive(r#""Hello".endsWith("HELLO")"#, 0)]
fn test_string_ends_with(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case::basic_true(r#""hello world".contains("lo wo")"#, 1)]
#[case::basic_false(r#""hello".contains("bye")"#, 0)]
#[case::empty_substring(r#""hello".contains("")"#, 1)]
#[case::full_match(r#""hello".contains("hello")"#, 1)]
#[case::at_start(r#""hello".contains("he")"#, 1)]
#[case::at_end(r#""hello".contains("lo")"#, 1)]
#[case::unicode(r#""こんにちは世界".contains("にちは")"#, 1)]
#[case::emoji(r#""Hello 👋 World 🌍".contains("👋")"#, 1)]
#[case::case_sensitive(r#""Hello".contains("hello")"#, 0)]
fn test_string_contains(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case::method_basic_match(r#""foobar".matches("foo.*")"#, 1)]
#[case::method_no_match(r#""hello".matches("world")"#, 0)]
#[case::function_basic_match(r#"matches("foobar", "foo.*")"#, 1)]
#[case::function_no_match(r#"matches("hello", "world")"#, 0)]
#[case::substring_match(r#""hello world".matches("wor")"#, 1)]
#[case::anchored_start_match(r#""foobar".matches("^foo")"#, 1)]
#[case::anchored_start_no_match(r#""foobar".matches("^bar")"#, 0)]
#[case::anchored_end_match(r#""foobar".matches("bar$")"#, 1)]
#[case::anchored_end_no_match(r#""foobar".matches("foo$")"#, 0)]
#[case::full_anchored_match(r#""foobar".matches("^foobar$")"#, 1)]
#[case::full_anchored_no_match(r#""foobar".matches("^foo$")"#, 0)]
#[case::character_class_digit(r#""abc123def".matches("[0-9]+")"#, 1)]
#[case::character_class_letter(r#""abc123def".matches("[a-z]+")"#, 1)]
#[case::quantifier_plus(r#""aaaa".matches("a+")"#, 1)]
#[case::quantifier_star(r#""".matches("a*")"#, 1)]
#[case::quantifier_question(r#""colour".matches("colou?r")"#, 1)]
#[case::quantifier_exact(r#""aaaa".matches("a{4}")"#, 1)]
#[case::quantifier_range(r#""aaaa".matches("a{3,5}")"#, 1)]
#[case::dot_wildcard(r#""a_b".matches("a.b")"#, 1)]
#[case::alternation(r#""cat".matches("cat|dog")"#, 1)]
#[case::unicode_pattern(r#""Hello 世界".matches("世界")"#, 1)]
#[case::emoji_pattern(r#""Hello 😀 World".matches("😀")"#, 1)]
#[case::email_pattern(r#""test@example.com".matches("[a-z]+@[a-z]+\\.[a-z]+")"#, 1)]
#[case::case_sensitive(r#""Hello".matches("hello")"#, 0)]
#[case::case_insensitive_flag(r#""Hello".matches("(?i)hello")"#, 1)]
fn test_string_matches(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ============================================================
// `in` Operator Tests
// ============================================================

#[rstest]
#[case::int_in_list(r#"2 in [1, 2, 3]"#, 1)]
#[case::int_not_in_list(r#"5 in [1, 2, 3]"#, 0)]
#[case::string_in_list(r#""b" in ["a", "b", "c"]"#, 1)]
#[case::string_not_in_list(r#""d" in ["a", "b"]"#, 0)]
#[case::bool_in_list(r#"true in [false, true]"#, 1)]
#[case::bool_not_in_list(r#"false in [true, true]"#, 0)]
#[case::empty_list(r#"1 in []"#, 0)]
#[case::double_in_list(r#"3.14 in [1.0, 2.0, 3.14]"#, 1)]
#[case::double_not_in_list(r#"3.14 in [1.0, 2.0]"#, 0)]
#[case::negative_int_in_list(r#"-5 in [-10, -5, 0, 5]"#, 1)]
#[case::nested_search(r#"2 in [1, 2, 3] in [true, false]"#, 1)]
fn test_in_operator_lists(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Map membership tests with JSON data parameter (tests that keys exist in maps)
#[rstest]
#[case::key_exists(
    r#""theme" in data.settings"#,
    r#"{"settings": {"theme": "dark", "lang": "en"}}"#,
    1
)]
#[case::key_missing(
    r#""color" in data.settings"#,
    r#"{"settings": {"theme": "dark", "lang": "en"}}"#,
    0
)]
#[case::key_with_null_value(
    r#""age" in data.user"#,
    r#"{"user": {"name": "Alice", "age": null}}"#,
    1
)]
#[case::key_with_string_value(
    r#""name" in data.user"#,
    r#"{"user": {"name": "Alice", "age": null}}"#,
    1
)]
fn test_in_operator_maps_with_data(
    #[case] expr: &str,
    #[case] data_json: &str,
    #[case] expected: i64,
) {
    let result = compile_and_execute_with_vars(expr, None, Some(data_json))
        .expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' with data should evaluate to {}",
        expr, expected
    );
}

// Map literal tests - testing both map literal creation and 'in' operator
#[rstest]
#[case::key_exists(r#""key" in {"key": "value", "other": 123}"#, 1)]
#[case::key_missing(r#""missing" in {"key": "value"}"#, 0)]
#[case::empty_map(r#""key" in {}"#, 0)]
#[case::multiple_types_name(r#""name" in {"name": "Alice", "age": 30, "active": true}"#, 1)]
#[case::multiple_types_age(r#""age" in {"name": "Alice", "age": 30, "active": true}"#, 1)]
#[case::multiple_types_missing(r#""score" in {"name": "Alice", "age": 30, "active": true}"#, 0)]
#[case::computed_values(r#""key" in {"key": 1 + 2, "other": 10 * 5}"#, 1)]
fn test_in_operator_map_literals(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Complex expressions with 'in' operator combined with input/data and logical operators
#[test]
fn test_in_operator_with_input_and_logical_ops() {
    let input = r#"{"items": [1, 2, 3, 4, 5]}"#;

    // Single membership test
    let result = compile_and_execute_with_vars(r#"3 in input.items"#, Some(input), None)
        .expect("Execution failed");
    assert_eq!(result, 1, "3 should be in input.items");

    // Combined with AND
    let result = compile_and_execute_with_vars(
        r#"(2 in input.items) && (6 in input.items)"#,
        Some(input),
        None,
    )
    .expect("Execution failed");
    assert_eq!(
        result, 0,
        "2 is in list but 6 is not, so AND should be false"
    );

    // Combined with OR
    let result = compile_and_execute_with_vars(
        r#"(2 in input.items) || (6 in input.items)"#,
        Some(input),
        None,
    )
    .expect("Execution failed");
    assert_eq!(result, 1, "2 is in list, so OR should be true");
}

// ============================================================
// Uint, Cross-Type Numeric & Bool Comparison Tests
// ============================================================

// Uint literal tests
#[rstest]
#[case::basic_uint("123u", 123)]
#[case::uppercase_u("456U", 456)]
#[case::zero("0u", 0)]
#[case::large("1000000000u", 1000000000)]
fn test_uint_literal(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Uint arithmetic tests
#[rstest]
#[case::add_basic("10u + 20u", 30)]
#[case::add_zero("5u + 0u", 5)]
#[case::sub_basic("20u - 10u", 10)]
#[case::sub_zero("5u - 0u", 5)]
#[case::sub_same("100u - 100u", 0)]
#[case::mul_basic("10u * 20u", 200)]
#[case::mul_zero("5u * 0u", 0)]
#[case::mul_one("100u * 1u", 100)]
#[case::div_basic("20u / 10u", 2)]
#[case::div_one("100u / 1u", 100)]
#[case::div_truncate("7u / 3u", 2)]
#[case::mod_basic("10u % 3u", 1)]
#[case::mod_zero("10u % 5u", 0)]
#[case::mod_large("100u % 7u", 2)]
fn test_uint_arithmetic(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Uint comparison tests
#[rstest]
#[case::eq_same("100u == 100u", 1)]
#[case::eq_different("100u == 200u", 0)]
#[case::ne_same("100u != 100u", 0)]
#[case::ne_different("100u != 200u", 1)]
#[case::lt_true("50u < 100u", 1)]
#[case::lt_false("100u < 50u", 0)]
#[case::lt_equal("100u < 100u", 0)]
#[case::gt_true("100u > 50u", 1)]
#[case::gt_false("50u > 100u", 0)]
#[case::gt_equal("100u > 100u", 0)]
#[case::lte_less("50u <= 100u", 1)]
#[case::lte_equal("100u <= 100u", 1)]
#[case::lte_greater("100u <= 50u", 0)]
#[case::gte_greater("100u >= 50u", 1)]
#[case::gte_equal("100u >= 100u", 1)]
#[case::gte_less("50u >= 100u", 0)]
fn test_uint_comparisons(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Cross-type numeric equality tests (CEL spec: numeric types on continuous number line)
#[rstest]
#[case::int_uint_equal("1 == 1u", 1)]
#[case::int_uint_different("1 == 2u", 0)]
#[case::int_uint_ne_same("1 != 1u", 0)]
#[case::int_uint_ne_different("1 != 2u", 1)]
#[case::uint_int_equal("5u == 5", 1)]
#[case::uint_int_different("5u == 10", 0)]
#[case::int_double_equal("5 == 5.0", 1)]
#[case::int_double_different("5 == 5.5", 0)]
#[case::uint_double_equal("10u == 10.0", 1)]
#[case::uint_double_different("10u == 10.5", 0)]
#[case::double_uint_equal("20.0 == 20u", 1)]
fn test_cross_type_equality(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Cross-type numeric ordering tests (CEL spec supports runtime ordering across int, uint, double)
#[rstest]
#[case::int_negative_lt_uint("-1 < 1u", 1)]
#[case::int_positive_lt_uint("5 < 10u", 1)]
#[case::int_gt_uint("10 > 5u", 1)]
#[case::int_lt_uint_false("10 < 5u", 0)]
#[case::uint_gt_int("10u > 5", 1)]
#[case::uint_lt_int("5u < 10", 1)]
#[case::int_lt_double("5 < 10.0", 1)]
#[case::uint_lt_double("5u < 10.0", 1)]
#[case::uint_gt_double("100u > 50.0", 1)]
#[case::uint_lt_double_false("100u < 50.0", 0)]
#[case::double_lt_uint("5.0 < 10u", 1)]
#[case::double_gt_uint("100.0 > 50u", 1)]
#[case::int_lte_uint_equal("5 <= 5u", 1)]
#[case::uint_gte_int_equal("5u >= 5", 1)]
fn test_cross_type_ordering(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Complex uint expressions
#[rstest]
#[case::precedence("10u + 20u * 2u", 50)] // 10 + 40
#[case::parentheses("(10u + 20u) * 2u", 60)]
#[case::mixed_ops("100u - 20u / 4u", 95)] // 100 - 5
#[case::comparison_chain("5u < 10u && 10u < 20u", 1)]
#[case::ternary_uint("true ? 10u : 20u", 10)]
#[case::ternary_uint_false("false ? 10u : 20u", 20)]
fn test_uint_complex_expressions(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// String comparison tests
#[rstest]
#[case::lt_true("\"a\" < \"b\"", 1)]
#[case::lt_false("\"b\" < \"a\"", 0)]
#[case::lt_equal("\"a\" < \"a\"", 0)]
#[case::lt_empty_to_nonempty("\"\" < \"a\"", 1)]
#[case::gt_true("\"b\" > \"a\"", 1)]
#[case::gt_false("\"a\" > \"b\"", 0)]
#[case::gt_equal("\"a\" > \"a\"", 0)]
#[case::lte_less("\"a\" <= \"b\"", 1)]
#[case::lte_equal("\"a\" <= \"a\"", 1)]
#[case::lte_greater("\"b\" <= \"a\"", 0)]
#[case::gte_greater("\"b\" >= \"a\"", 1)]
#[case::gte_equal("\"a\" >= \"a\"", 1)]
#[case::gte_less("\"a\" >= \"b\"", 0)]
#[case::lt_case("\"Abc\" < \"aBC\"", 1)] // A < a in lexicographic order
#[case::gt_case("\"abc\" > \"aBc\"", 1)] // a > B in lexicographic order
fn test_string_comparisons(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Boolean comparison tests
#[rstest]
#[case::lt_false_true("false < true", 1)]
#[case::lt_true_false("true < false", 0)]
#[case::lt_false_false("false < false", 0)]
#[case::lt_true_true("true < true", 0)]
#[case::gt_true_false("true > false", 1)]
#[case::gt_false_true("false > true", 0)]
#[case::gt_false_false("false > false", 0)]
#[case::lte_false_true("false <= true", 1)]
#[case::lte_false_false("false <= false", 1)]
#[case::lte_true_false("false <= true", 1)]
#[case::gte_true_false("true >= false", 1)]
#[case::gte_true_true("true >= true", 1)]
#[case::gte_false_true("false >= true", 0)]
fn test_bool_comparisons(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Map equality tests with mixed numeric types
#[rstest]
#[case::mixed_keys_and_values("{1: 1.0, 2u: 3u} == {1u: 1, 2: 3.0}", 1)]
#[case::int_uint_keys("{1: 'a', 2: 'b'} == {1u: 'a', 2u: 'b'}", 1)]
#[case::different_values("{1: 1.0} == {1u: 2.0}", 0)]
fn test_map_cross_type_equality(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// List equality tests with mixed numeric types
#[rstest]
#[case::int_uint_equal("[1, 2] == [1u, 2u]", 1)]
#[case::int_double_equal("[1, 2.0] == [1.0, 2]", 1)]
#[case::mixed_types("[1, 2u, 3.0] == [1.0, 2, 3u]", 1)]
fn test_list_cross_type_equality(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ============================================================
// Struct Literal & Equality Tests
// ============================================================

#[test]
fn test_struct_empty() {
    // Empty struct should compile and create a map with just __type__ field
    let result = compile_and_execute_json("TestAllTypes{}");
    assert!(result.is_ok(), "Empty struct should compile successfully");
    let json = result.unwrap();
    assert!(
        json.is_object(),
        "Struct should be represented as a map/object"
    );
    let obj = json.as_object().unwrap();
    assert!(
        obj.contains_key("__type__"),
        "Struct should have __type__ field"
    );
    assert_eq!(obj.get("__type__").unwrap(), "TestAllTypes");
}

#[test]
fn test_struct_wrapper_bool() {
    // google.protobuf.BoolValue wrapper type
    let result = compile_and_execute_json("google.protobuf.BoolValue{value: true}");
    assert!(
        result.is_ok(),
        "BoolValue struct should compile successfully"
    );
    let json = result.unwrap();
    assert!(json.is_object());
    let obj = json.as_object().unwrap();
    assert_eq!(obj.get("__type__").unwrap(), "google.protobuf.BoolValue");
    assert_eq!(obj.get("value").unwrap(), &serde_json::Value::Bool(true));
}

#[test]
fn test_struct_wrapper_int32() {
    // google.protobuf.Int32Value wrapper type
    let result = compile_and_execute_json("google.protobuf.Int32Value{value: 123}");
    assert!(
        result.is_ok(),
        "Int32Value struct should compile successfully"
    );
    let json = result.unwrap();
    let obj = json.as_object().unwrap();
    assert_eq!(obj.get("__type__").unwrap(), "google.protobuf.Int32Value");
    assert_eq!(obj.get("value").unwrap(), &serde_json::json!(123));
}

#[test]
fn test_struct_multiple_fields() {
    // Struct with multiple fields
    let result =
        compile_and_execute_json("TestAllTypes{single_int64: 1234, single_string: '1234'}");
    assert!(
        result.is_ok(),
        "Multi-field struct should compile successfully"
    );
    let json = result.unwrap();
    let obj = json.as_object().unwrap();
    assert_eq!(obj.get("__type__").unwrap(), "TestAllTypes");
    assert_eq!(obj.get("single_int64").unwrap(), &serde_json::json!(1234));
    assert_eq!(obj.get("single_string").unwrap(), "1234");
}

// Struct equality tests
#[rstest]
#[case::wrapper_bool_equal(
    "google.protobuf.BoolValue{value: true} == google.protobuf.BoolValue{value: true}",
    1
)]
#[case::wrapper_bool_not_equal(
    "google.protobuf.BoolValue{value: true} == google.protobuf.BoolValue{value: false}",
    0
)]
#[case::wrapper_int_equal(
    "google.protobuf.Int32Value{value: 123} == google.protobuf.Int32Value{value: 123}",
    1
)]
#[case::wrapper_int_not_equal(
    "google.protobuf.Int32Value{value: 123} == google.protobuf.Int32Value{value: 456}",
    0
)]
#[case::empty_structs_equal("TestAllTypes{} == TestAllTypes{}", 1)]
#[case::multi_field_equal(
    "TestAllTypes{single_int64: 1234, single_string: '1234'} == TestAllTypes{single_int64: 1234, single_string: '1234'}",
    1
)]
#[case::multi_field_not_equal(
    "TestAllTypes{single_int64: 1234} == TestAllTypes{single_int64: 5678}",
    0
)]
fn test_struct_equality(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ============================================================
// Container Resolution Tests
// ============================================================

#[test]
fn test_container_no_schema_no_container() {
    // Without schema or container, unqualified names should still work (treated as arbitrary structs)
    let result = compile_with_container("MyType{field: 42}", None, None);
    assert!(
        result.is_ok(),
        "Should compile unqualified struct name without schema"
    );
}

#[test]
fn test_container_with_container_but_no_schema() {
    // With container but no schema, resolution should fall back to using the name as-is
    // This is a graceful degradation case
    let result = compile_with_container("MyType{field: 42}", Some("com.example"), None);
    assert!(
        result.is_ok(),
        "Should compile with container but no schema (graceful degradation)"
    );
}

// Note: More comprehensive tests with proto descriptors would require building
// the proto files first with protoc. For now, these tests verify the basic
// container resolution logic compiles correctly.

// ============================================================
// Extension Function Tests
// ============================================================

#[test]
fn test_extension_global_call() {
    // Register myFunc(x) that doubles its argument, call myFunc(21) -> 42.
    let logger = create_test_logger();
    let decl = ExtensionDecl {
        namespace: None,
        function: "myFunc".to_string(),
        receiver_style: false,
        global_style: true,
        num_args: 1,
    };
    let mut engine = CelEngine::new(logger);
    engine.register_extension(decl.clone(), |args| {
        let x = args[0].as_i64().unwrap_or(0);
        Ok(serde_json::Value::Number((x * 2).into()))
    });

    let wasm = compile_cel_to_wasm("myFunc(21)", options_with_ext(decl)).expect("compile failed");
    let result = engine.execute(&wasm, None).expect("execute failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 42);
}

#[test]
fn test_extension_namespaced_call() {
    // Register math.abs(x), call math.abs(-7) -> 7.
    let logger = create_test_logger();
    let decl = ExtensionDecl {
        namespace: Some("math".to_string()),
        function: "abs".to_string(),
        receiver_style: false,
        global_style: true,
        num_args: 1,
    };
    let mut engine = CelEngine::new(logger);
    engine.register_extension(decl.clone(), |args| {
        let x = args[0].as_i64().unwrap_or(0);
        Ok(serde_json::Value::Number(x.abs().into()))
    });

    let wasm = compile_cel_to_wasm("math.abs(-7)", options_with_ext(decl)).expect("compile failed");
    let result = engine.execute(&wasm, None).expect("execute failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 7);
}

#[test]
fn test_extension_receiver_style_call() {
    // Register reverse(x) with receiver_style, call "hello".reverse() -> "olleh".
    let logger = create_test_logger();
    let decl = ExtensionDecl {
        namespace: None,
        function: "reverse".to_string(),
        receiver_style: true,
        global_style: false,
        num_args: 1,
    };
    let mut engine = CelEngine::new(logger);
    engine.register_extension(decl.clone(), |args| {
        let s = args[0]
            .as_str()
            .unwrap_or("")
            .chars()
            .rev()
            .collect::<String>();
        Ok(serde_json::Value::String(s))
    });

    let wasm = compile_cel_to_wasm(r#""hello".reverse()"#, options_with_ext(decl))
        .expect("compile failed");
    let result = engine.execute(&wasm, None).expect("execute failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_str().unwrap(), "olleh");
}

#[test]
fn test_extension_both_call_styles() {
    // Register reverse with both receiver and global style.
    // "hello".reverse() and reverse("hello") should both give "olleh".
    let logger = create_test_logger();
    let decl = ExtensionDecl {
        namespace: None,
        function: "rev".to_string(),
        receiver_style: true,
        global_style: true,
        num_args: 1,
    };
    let mut engine = CelEngine::new(logger);
    engine.register_extension(decl.clone(), |args| {
        let s = args[0]
            .as_str()
            .unwrap_or("")
            .chars()
            .rev()
            .collect::<String>();
        Ok(serde_json::Value::String(s))
    });

    let wasm_recv = compile_cel_to_wasm(r#""hello".rev()"#, options_with_ext(decl.clone()))
        .expect("compile receiver failed");
    let result_recv = engine
        .execute(&wasm_recv, None)
        .expect("execute receiver failed");
    let v_recv: serde_json::Value = serde_json::from_str(&result_recv).unwrap();

    let wasm_glob = compile_cel_to_wasm(r#"rev("hello")"#, options_with_ext(decl))
        .expect("compile global failed");
    let result_glob = engine
        .execute(&wasm_glob, None)
        .expect("execute global failed");
    let v_glob: serde_json::Value = serde_json::from_str(&result_glob).unwrap();

    assert_eq!(v_recv.as_str().unwrap(), "olleh");
    assert_eq!(v_glob.as_str().unwrap(), "olleh");
}

#[test]
fn test_extension_multi_arg() {
    // Register add3(a, b, c), call add3(1, 2, 3) -> 6.
    let (engine, decl) = make_engine_with_sum(None, "add3", 3, false, true);

    let wasm =
        compile_cel_to_wasm("add3(1, 2, 3)", options_with_ext(decl)).expect("compile failed");
    let result = engine.execute(&wasm, None).expect("execute failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 6);
}

#[test]
fn test_extension_arity_mismatch_is_compile_error() {
    // Register myFunc with num_args=1, try to compile myFunc(1, 2) -> error.
    let (_engine, decl) = make_engine_with_sum(None, "myFunc", 1, false, true);

    let result = compile_cel_to_wasm("myFunc(1, 2)", options_with_ext(decl));
    assert!(
        result.is_err(),
        "Arity mismatch should produce a compile error"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("expects") || msg.contains("argument"),
        "Error message should mention argument count, got: {msg}"
    );
}

#[test]
fn test_extension_unknown_function_is_compile_error() {
    // No extensions registered; calling unknown() should error.
    let options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: create_test_logger(),
        extensions: vec![],
    };

    let result = compile_cel_to_wasm("unknown(1)", options);
    assert!(
        result.is_err(),
        "Calling an unknown function should produce a compile error"
    );
}

#[test]
fn test_extension_wrong_call_style_is_compile_error() {
    // Register myFunc with global_style only; try receiver-style -> error.
    let (_engine, decl) = make_engine_with_sum(None, "myFunc", 1, false, true);

    let result = compile_cel_to_wasm("42.myFunc()", options_with_ext(decl));
    assert!(
        result.is_err(),
        "Using receiver-style on a global-only extension should error"
    );
}

#[test]
fn test_extension_with_bindings() {
    // math.abs(x) where x comes from bindings.
    let logger = create_test_logger();
    let decl = ExtensionDecl {
        namespace: Some("math".to_string()),
        function: "abs".to_string(),
        receiver_style: false,
        global_style: true,
        num_args: 1,
    };
    let mut engine = CelEngine::new(logger);
    engine.register_extension(decl.clone(), |args| {
        let x = args[0].as_i64().unwrap_or(0);
        Ok(serde_json::Value::Number(x.abs().into()))
    });

    let wasm =
        compile_cel_to_wasm("math.abs(input)", options_with_ext(decl)).expect("compile failed");
    let bindings = r#"{"input": -99}"#;
    let result = engine
        .execute(&wasm, Some(bindings))
        .expect("execute failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 99);
}
