// Integration tests for variable access (input/data) and field access.

mod common;
use common::*;

// ========================================
// Variable Access Tests (PR #4)
// ========================================

#[test]
fn test_input_variable_positive() {
    // Test accessing input variable with a positive integer
    let result =
        compile_and_execute_with_input_data("input", Some("42"), None).expect("Failed to execute");
    assert_eq!(result, 42, "input should return 42");
}

#[test]
fn test_input_variable_negative() {
    // Test accessing input variable with a negative integer
    let result =
        compile_and_execute_with_input_data("input", Some("-10"), None).expect("Failed to execute");
    assert_eq!(result, -10, "input should return -10");
}

#[test]
fn test_input_variable_zero() {
    // Test accessing input variable with zero
    let result =
        compile_and_execute_with_input_data("input", Some("0"), None).expect("Failed to execute");
    assert_eq!(result, 0, "input should return 0");
}

#[test]
fn test_data_variable_positive() {
    // Test accessing data variable with a positive integer
    let result =
        compile_and_execute_with_input_data("data", None, Some("100")).expect("Failed to execute");
    assert_eq!(result, 100, "data should return 100");
}

#[test]
fn test_data_variable_negative() {
    // Test accessing data variable with a negative integer
    let result =
        compile_and_execute_with_input_data("data", None, Some("-50")).expect("Failed to execute");
    assert_eq!(result, -50, "data should return -50");
}

#[test]
fn test_input_and_data_addition() {
    // Test using both input and data in an expression
    let result = compile_and_execute_with_input_data("input + data", Some("10"), Some("20"))
        .expect("Failed to execute");
    assert_eq!(result, 30, "input + data should return 30");
}

#[test]
fn test_input_and_data_multiplication() {
    // Test multiplication with input and data
    let result = compile_and_execute_with_input_data("input * data", Some("5"), Some("7"))
        .expect("Failed to execute");
    assert_eq!(result, 35, "input * data should return 35");
}

#[test]
fn test_input_in_complex_expression() {
    // Test input in a more complex expression
    let result = compile_and_execute_with_input_data("input * 2 + 10", Some("5"), None)
        .expect("Failed to execute");
    assert_eq!(result, 20, "input * 2 + 10 should return 20");
}

#[test]
fn test_data_in_complex_expression() {
    // Test data in a more complex expression
    let result = compile_and_execute_with_input_data("(data - 5) * 3", None, Some("10"))
        .expect("Failed to execute");
    assert_eq!(result, 15, "(data - 5) * 3 should return 15");
}

#[test]
fn test_input_variable_i64_max() {
    // Test with i64::MAX
    let max = i64::MAX;
    let input_json = format!("{}", max);
    let result = compile_and_execute_with_input_data("input", Some(&input_json), None)
        .expect("Failed to execute");
    assert_eq!(result, max, "input should return i64::MAX");
}

#[test]
fn test_input_variable_i64_min() {
    // Test with i64::MIN
    let min = i64::MIN;
    let input_json = format!("{}", min);
    let result = compile_and_execute_with_input_data("input", Some(&input_json), None)
        .expect("Failed to execute");
    assert_eq!(result, min, "input should return i64::MIN");
}

// ========================================
// Field Access Tests
// ========================================

#[test]
fn test_simple_field_access() {
    // Test accessing a field from input object
    let input_json = r#"{"age": 42}"#;
    let result = compile_and_execute_with_input_data("input.age", Some(input_json), None)
        .expect("Failed to execute");
    assert_eq!(result, 42, "input.age should return 42");
}

#[test]
fn test_nested_field_access() {
    // Test accessing nested fields
    let input_json = r#"{"user": {"age": 30}}"#;
    let result = compile_and_execute_with_input_data("input.user.age", Some(input_json), None)
        .expect("Failed to execute");
    assert_eq!(result, 30, "input.user.age should return 30");
}

#[test]
fn test_field_access_with_data() {
    // Test field access on data variable
    let data_json = r#"{"count": 100}"#;
    let result = compile_and_execute_with_input_data("data.count", None, Some(data_json))
        .expect("Failed to execute");
    assert_eq!(result, 100, "data.count should return 100");
}

#[test]
fn test_field_access_in_expression() {
    // Test using field access in arithmetic
    let input_json = r#"{"x": 10}"#;
    let result = compile_and_execute_with_input_data("input.x * 2 + 5", Some(input_json), None)
        .expect("Failed to execute");
    assert_eq!(result, 25, "input.x * 2 + 5 should return 25");
}

#[test]
fn test_multiple_field_access() {
    // Test accessing fields from both input and data
    let input_json = r#"{"a": 10}"#;
    let data_json = r#"{"b": 20}"#;
    let result =
        compile_and_execute_with_input_data("input.a + data.b", Some(input_json), Some(data_json))
            .expect("Failed to execute");
    assert_eq!(result, 30, "input.a + data.b should return 30");
}

#[test]
fn test_deeply_nested_field_access() {
    // Test accessing deeply nested fields
    let input_json = r#"{"level1": {"level2": {"level3": {"value": 99}}}}"#;
    let result = compile_and_execute_with_input_data(
        "input.level1.level2.level3.value",
        Some(input_json),
        None,
    )
    .expect("Failed to execute");
    assert_eq!(result, 99, "deeply nested field should return 99");
}
