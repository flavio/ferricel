// Integration tests for struct literal creation and struct equality.

mod common;
use common::*;

use rstest::rstest;

// ============================================================
// Struct Literal & Equality Tests
// ============================================================

#[test]
fn test_struct_empty() {
    // Empty struct should compile and create a map with just __type__ field
    let result = compile_and_execute("TestAllTypes{}");
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
    let result = compile_and_execute("google.protobuf.BoolValue{value: true}");
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
    let result = compile_and_execute("google.protobuf.Int32Value{value: 123}");
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
    let result = compile_and_execute("TestAllTypes{single_int64: 1234, single_string: '1234'}");
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
