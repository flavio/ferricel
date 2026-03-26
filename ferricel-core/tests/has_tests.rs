// Integration tests for the has() macro.

mod common;
use common::*;

use rstest::rstest;

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
