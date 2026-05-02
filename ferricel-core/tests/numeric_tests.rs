// Integration tests for uint literals/arithmetic/comparisons, cross-type numeric equality
// and ordering, string comparisons, boolean comparisons, and cross-type map/list equality.

use crate::common::*;

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
#[case::eq_same("100u == 100u", true)]
#[case::eq_different("100u == 200u", false)]
#[case::ne_same("100u != 100u", false)]
#[case::ne_different("100u != 200u", true)]
#[case::lt_true("50u < 100u", true)]
#[case::lt_false("100u < 50u", false)]
#[case::lt_equal("100u < 100u", false)]
#[case::gt_true("100u > 50u", true)]
#[case::gt_false("50u > 100u", false)]
#[case::gt_equal("100u > 100u", false)]
#[case::lte_less("50u <= 100u", true)]
#[case::lte_equal("100u <= 100u", true)]
#[case::lte_greater("100u <= 50u", false)]
#[case::gte_greater("100u >= 50u", true)]
#[case::gte_equal("100u >= 100u", true)]
#[case::gte_less("50u >= 100u", false)]
fn test_uint_comparisons(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Cross-type numeric equality tests (CEL spec: numeric types on continuous number line)
#[rstest]
#[case::int_uint_equal("1 == 1u", true)]
#[case::int_uint_different("1 == 2u", false)]
#[case::int_uint_ne_same("1 != 1u", false)]
#[case::int_uint_ne_different("1 != 2u", true)]
#[case::uint_int_equal("5u == 5", true)]
#[case::uint_int_different("5u == 10", false)]
#[case::int_double_equal("5 == 5.0", true)]
#[case::int_double_different("5 == 5.5", false)]
#[case::uint_double_equal("10u == 10.0", true)]
#[case::uint_double_different("10u == 10.5", false)]
#[case::double_uint_equal("20.0 == 20u", true)]
fn test_cross_type_equality(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Cross-type numeric ordering tests (CEL spec supports runtime ordering across int, uint, double)
#[rstest]
#[case::int_negative_lt_uint("-1 < 1u", true)]
#[case::int_positive_lt_uint("5 < 10u", true)]
#[case::int_gt_uint("10 > 5u", true)]
#[case::int_lt_uint_false("10 < 5u", false)]
#[case::uint_gt_int("10u > 5", true)]
#[case::uint_lt_int("5u < 10", true)]
#[case::int_lt_double("5 < 10.0", true)]
#[case::uint_lt_double("5u < 10.0", true)]
#[case::uint_gt_double("100u > 50.0", true)]
#[case::uint_lt_double_false("100u < 50.0", false)]
#[case::double_lt_uint("5.0 < 10u", true)]
#[case::double_gt_uint("100.0 > 50u", true)]
#[case::int_lte_uint_equal("5 <= 5u", true)]
#[case::uint_gte_int_equal("5u >= 5", true)]
fn test_cross_type_ordering(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
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

#[test]
fn test_uint_complex_comparison_chain() {
    let result =
        compile_and_execute_bool("5u < 10u && 10u < 20u").expect("Failed to compile and execute");
    assert!(result, "5u < 10u && 10u < 20u should be true");
}

// String comparison tests
#[rstest]
#[case::lt_true("\"a\" < \"b\"", true)]
#[case::lt_false("\"b\" < \"a\"", false)]
#[case::lt_equal("\"a\" < \"a\"", false)]
#[case::lt_empty_to_nonempty("\"\" < \"a\"", true)]
#[case::gt_true("\"b\" > \"a\"", true)]
#[case::gt_false("\"a\" > \"b\"", false)]
#[case::gt_equal("\"a\" > \"a\"", false)]
#[case::lte_less("\"a\" <= \"b\"", true)]
#[case::lte_equal("\"a\" <= \"a\"", true)]
#[case::lte_greater("\"b\" <= \"a\"", false)]
#[case::gte_greater("\"b\" >= \"a\"", true)]
#[case::gte_equal("\"a\" >= \"a\"", true)]
#[case::gte_less("\"a\" >= \"b\"", false)]
#[case::lt_case("\"Abc\" < \"aBC\"", true)] // A < a in lexicographic order
#[case::gt_case("\"abc\" > \"aBc\"", true)] // a > B in lexicographic order
fn test_string_comparisons(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Boolean comparison tests
#[rstest]
#[case::lt_false_true("false < true", true)]
#[case::lt_true_false("true < false", false)]
#[case::lt_false_false("false < false", false)]
#[case::lt_true_true("true < true", false)]
#[case::gt_true_false("true > false", true)]
#[case::gt_false_true("false > true", false)]
#[case::gt_false_false("false > false", false)]
#[case::lte_false_true("false <= true", true)]
#[case::lte_false_false("false <= false", true)]
#[case::lte_true_false("false <= true", true)]
#[case::gte_true_false("true >= false", true)]
#[case::gte_true_true("true >= true", true)]
#[case::gte_false_true("false >= true", false)]
fn test_bool_comparisons(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// Map equality tests with mixed numeric types
#[rstest]
#[case::mixed_keys_and_values("{1: 1.0, 2u: 3u} == {1u: 1, 2: 3.0}", true)]
#[case::int_uint_keys("{1: 'a', 2: 'b'} == {1u: 'a', 2u: 'b'}", true)]
#[case::different_values("{1: 1.0} == {1u: 2.0}", false)]
fn test_map_cross_type_equality(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// List equality tests with mixed numeric types
#[rstest]
#[case::int_uint_equal("[1, 2] == [1u, 2u]", true)]
#[case::int_double_equal("[1, 2.0] == [1.0, 2]", true)]
#[case::mixed_types("[1, 2u, 3.0] == [1.0, 2, 3u]", true)]
fn test_list_cross_type_equality(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}
