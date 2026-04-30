// Integration tests for the `in` operator (list membership and map key presence).

mod common;
use common::*;

use rstest::rstest;

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
    let result = compile_and_execute_with_input_data(expr, None, Some(data_json))
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
    let result = compile_and_execute_with_input_data(r#"3 in input.items"#, Some(input), None)
        .expect("Execution failed");
    assert_eq!(result, 1, "3 should be in input.items");

    // Combined with AND
    let result = compile_and_execute_with_input_data(
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
    let result = compile_and_execute_with_input_data(
        r#"(2 in input.items) || (6 in input.items)"#,
        Some(input),
        None,
    )
    .expect("Execution failed");
    assert_eq!(result, 1, "2 is in list, so OR should be true");
}
