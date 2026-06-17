// Integration tests for JSON serialization of CEL evaluation results.

use crate::common::*;

#[test]
fn test_json_output_integer() {
    let result = compile_and_execute("42").expect("Failed to compile and execute");
    assert_eq!(result, serde_json::json!(42));
}

#[test]
fn test_json_output_boolean_true() {
    let result = compile_and_execute("5 > 3").expect("Failed to compile and execute");
    assert_eq!(result, serde_json::json!(true));
}

#[test]
fn test_json_output_boolean_false() {
    let result = compile_and_execute("5 < 3").expect("Failed to compile and execute");
    assert_eq!(result, serde_json::json!(false));
}

#[test]
fn test_json_output_negative_integer() {
    let result = compile_and_execute("-123").expect("Failed to compile and execute");
    assert_eq!(result, serde_json::json!(-123));
}

#[test]
fn test_json_output_arithmetic_result() {
    let result = compile_and_execute("10 + 20 * 2").expect("Failed to compile and execute");
    assert_eq!(result, serde_json::json!(50));
}
