use ferricel_core::runtime;
use ferricel_types::LogLevel;
use rstest::rstest;

use crate::common::*;

// ---------------------------------------------------------------------------
// cel.block — basic slot bindings
// ---------------------------------------------------------------------------

#[rstest]
#[case::single_slot_body_references_it("cel.block([1 + 1], cel.index(0)) == 2")]
#[case::multiple_independent_slots(
    "cel.block([1, 2, 3], cel.index(0) + cel.index(1) + cel.index(2)) == 6"
)]
#[case::later_slot_references_earlier_slot(
    "cel.block([1, cel.index(0) + 1, cel.index(1) + 1], cel.index(2)) == 3"
)]
#[case::body_can_reference_same_slot_multiple_times(
    "cel.block([5], cel.index(0) * cel.index(0)) == 25"
)]
#[case::string_slots(
    "cel.block(['hello', ' world'], cel.index(0) + cel.index(1)) == 'hello world'"
)]
#[case::boolean_slot("cel.block([1 == 1], cel.index(0)) == true")]
#[case::null_slot("cel.block([null], cel.index(0) == null)")]
fn test_cel_block(#[case] expr: &str) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result,
        serde_json::Value::Bool(true),
        "Expression '{}' should be true",
        expr
    );
}

// ---------------------------------------------------------------------------
// cel.block — with variables
// ---------------------------------------------------------------------------

#[test]
fn test_cel_block_with_variable() {
    let wasm = compile_with_container("cel.block([x + 1], cel.index(0)) == 6", None, None).unwrap();
    let bindings = serde_json::to_string(&serde_json::json!({ "x": 5 })).unwrap();
    let result: serde_json::Value = serde_json::from_str(
        &runtime::Builder::new()
            .with_logger(create_test_logger())
            .with_log_level(LogLevel::Info)
            .with_wasm(wasm)
            .build()
            .unwrap()
            .eval(Some(&bindings))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result, serde_json::Value::Bool(true));
}

#[test]
fn test_cel_block_two_variables() {
    let wasm = compile_with_container(
        "cel.block([x, y], cel.index(0) + cel.index(1)) == 3",
        None,
        None,
    )
    .unwrap();
    let bindings = serde_json::to_string(&serde_json::json!({ "x": 1, "y": 2 })).unwrap();
    let result: serde_json::Value = serde_json::from_str(
        &runtime::Builder::new()
            .with_logger(create_test_logger())
            .with_log_level(LogLevel::Info)
            .with_wasm(wasm)
            .build()
            .unwrap()
            .eval(Some(&bindings))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result, serde_json::Value::Bool(true));
}

// ---------------------------------------------------------------------------
// cel.block — slot value reuse (CSE semantics)
// ---------------------------------------------------------------------------

#[test]
fn test_cel_block_slot_reuse() {
    // Slot 0 is computed once; body uses it twice. Should produce same value both times.
    let wasm = compile_with_container(
        "cel.block([x + 1], cel.index(0) + cel.index(0))",
        None,
        None,
    )
    .unwrap();
    let bindings = serde_json::to_string(&serde_json::json!({ "x": 3 })).unwrap();
    let result: serde_json::Value = serde_json::from_str(
        &runtime::Builder::new()
            .with_logger(create_test_logger())
            .with_log_level(LogLevel::Info)
            .with_wasm(wasm)
            .build()
            .unwrap()
            .eval(Some(&bindings))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result, serde_json::json!(8));
}

// ---------------------------------------------------------------------------
// cel.block — inside larger expressions
// ---------------------------------------------------------------------------

#[test]
fn test_cel_block_in_ternary() {
    let result = compile_and_execute("cel.block([1 > 0], cel.index(0) ? 'yes' : 'no') == 'yes'")
        .expect("should succeed");
    assert_eq!(result, serde_json::Value::Bool(true));
}
