use crate::common::*;
use rstest::rstest;

// ---------------------------------------------------------------------------
// cel.block — basic slot bindings
// ---------------------------------------------------------------------------

#[rstest]
// Single slot, body references it
#[case("cel.block([1 + 1], cel.index(0)) == 2")]
// Multiple independent slots
#[case("cel.block([1, 2, 3], cel.index(0) + cel.index(1) + cel.index(2)) == 6")]
// Later slot references earlier slot
#[case("cel.block([1, cel.index(0) + 1, cel.index(1) + 1], cel.index(2)) == 3")]
// Body can reference same slot multiple times
#[case("cel.block([5], cel.index(0) * cel.index(0)) == 25")]
// String slots
#[case("cel.block(['hello', ' world'], cel.index(0) + cel.index(1)) == 'hello world'")]
// Boolean slot
#[case("cel.block([1 == 1], cel.index(0)) == true")]
// Null slot
#[case("cel.block([null], cel.index(0) == null)")]
fn test_cel_block(#[case] expr: &str) {
    let result = compile_and_execute_json(expr).expect("Failed to compile and execute");
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
    let result = compile_and_execute_with_container(
        "cel.block([x + 1], cel.index(0)) == 6",
        None,
        serde_json::json!({ "x": 5 }),
    )
    .expect("should succeed");
    assert_eq!(result, serde_json::Value::Bool(true));
}

#[test]
fn test_cel_block_two_variables() {
    let result = compile_and_execute_with_container(
        "cel.block([x, y], cel.index(0) + cel.index(1)) == 3",
        None,
        serde_json::json!({ "x": 1, "y": 2 }),
    )
    .expect("should succeed");
    assert_eq!(result, serde_json::Value::Bool(true));
}

// ---------------------------------------------------------------------------
// cel.block — slot value reuse (CSE semantics)
// ---------------------------------------------------------------------------

#[test]
fn test_cel_block_slot_reuse() {
    // Slot 0 is computed once; body uses it twice. Should produce same value both times.
    let result = compile_and_execute_with_container(
        "cel.block([x + 1], cel.index(0) + cel.index(0))",
        None,
        serde_json::json!({ "x": 3 }),
    )
    .expect("should succeed");
    assert_eq!(result, serde_json::json!(8));
}

// ---------------------------------------------------------------------------
// cel.block — inside larger expressions
// ---------------------------------------------------------------------------

#[test]
fn test_cel_block_in_ternary() {
    let result =
        compile_and_execute_json("cel.block([1 > 0], cel.index(0) ? 'yes' : 'no') == 'yes'")
            .expect("should succeed");
    assert_eq!(result, serde_json::Value::Bool(true));
}
