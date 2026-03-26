// Integration tests for uint literals/arithmetic/comparisons, cross-type numeric equality
// and ordering, string comparisons, boolean comparisons, and cross-type map/list equality.

mod common;
use common::*;

use rstest::rstest;

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
