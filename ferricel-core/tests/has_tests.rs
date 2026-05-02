// Integration tests for the has() macro.

use crate::common::*;

use rstest::rstest;

// ============================================================================
// HAS MACRO TESTS
// ============================================================================

#[rstest]
#[case(r#"{"name": "Alice", "age": 30}"#, "has(input.name)", true)]
#[case(r#"{"name": "Alice", "age": 30}"#, "has(input.age)", true)]
#[case(r#"{"name": "Alice"}"#, "has(input.age)", false)]
#[case(r#"{"name": "Alice"}"#, "has(input.email)", false)]
#[case(r#"{}"#, "has(input.anything)", false)]
fn test_has_macro_basic(#[case] input_json: &str, #[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_with_input_data(expr, Some(input_json), None)
        .expect("Failed to execute")
        .as_bool()
        .unwrap_or_else(|| panic!("Expected bool result for '{}'", expr));
    assert_eq!(
        result, expected,
        "Expression '{}' with input {} should evaluate to {}",
        expr, input_json, expected
    );
}

#[rstest]
#[case(r#"{"user": {"name": "Bob"}}"#, "has(input.user.name)", true)]
#[case(r#"{"user": {"name": "Bob"}}"#, "has(input.user.age)", false)]
#[case(r#"{"user": {}}"#, "has(input.user.name)", false)]
#[case(r#"{"a": {"b": {"c": 42}}}"#, "has(input.a.b.c)", true)]
#[case(r#"{"a": {"b": {}}}"#, "has(input.a.b.c)", false)]
fn test_has_macro_nested(#[case] input_json: &str, #[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_with_input_data(expr, Some(input_json), None)
        .expect("Failed to execute")
        .as_bool()
        .unwrap_or_else(|| panic!("Expected bool result for '{}'", expr));
    assert_eq!(
        result, expected,
        "Expression '{}' with input {} should evaluate to {}",
        expr, input_json, expected
    );
}

#[test]
fn test_has_macro_with_data_variable() {
    let data_json = r#"{"config": {"enabled": true}}"#;
    let result = compile_and_execute_with_input_data("has(data.config)", None, Some(data_json))
        .expect("Failed to execute")
        .as_bool()
        .expect("Expected bool result");
    assert!(result, "has(data.config) should return true");
}

#[test]
fn test_has_macro_with_null_value() {
    // Field exists but value is null - should return true
    let input_json = r#"{"nullable": null}"#;
    let result = compile_and_execute_with_input_data("has(input.nullable)", Some(input_json), None)
        .expect("Failed to execute")
        .as_bool()
        .expect("Expected bool result");
    assert!(
        result,
        "has(input.nullable) should return true even when value is null"
    );
}

#[rstest]
#[case(r#"{"age": 25}"#, "has(input.age) && input.age > 18", true)]
#[case(r#"{"age": 15}"#, "has(input.age) && input.age > 18", false)]
#[case(r#"{"age": 25}"#, "has(input.age) || has(input.name)", true)]
#[case(r#"{}"#, "has(input.age) || has(input.name)", false)]
#[case(r#"{"name": "Alice"}"#, "!has(input.age)", true)]
#[case(r#"{"age": 25}"#, "!has(input.missing)", true)]
fn test_has_macro_in_expressions(
    #[case] input_json: &str,
    #[case] expr: &str,
    #[case] expected: bool,
) {
    let result = compile_and_execute_with_input_data(expr, Some(input_json), None)
        .expect("Failed to execute")
        .as_bool()
        .unwrap_or_else(|| panic!("Expected bool result for '{}'", expr));
    assert_eq!(
        result, expected,
        "Expression '{}' with input {} should evaluate to {}",
        expr, input_json, expected
    );
}

#[rstest]
#[case(r#"{"a": 1, "b": 2}"#, "has(input.a) && has(input.b)", true)]
#[case(
    r#"{"a": 1, "b": 2}"#,
    "has(input.a) && has(input.b) && !has(input.c)",
    true
)]
#[case(r#"{"a": 1}"#, "has(input.a) && has(input.b)", false)]
#[case(
    r#"{"a": 1, "b": 2, "c": 3}"#,
    "has(input.a) && has(input.b) && has(input.c)",
    true
)]
fn test_has_macro_multiple_fields(
    #[case] input_json: &str,
    #[case] expr: &str,
    #[case] expected: bool,
) {
    let result = compile_and_execute_with_input_data(expr, Some(input_json), None)
        .expect("Failed to execute")
        .as_bool()
        .unwrap_or_else(|| panic!("Expected bool result for '{}'", expr));
    assert_eq!(
        result, expected,
        "Expression '{}' with input {} should evaluate to {}",
        expr, input_json, expected
    );
}
