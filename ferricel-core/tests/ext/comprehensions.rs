use crate::common::*;
use rstest::rstest;

// ─── exists(i, v, pred) ────────────────────────────────────────────────────

#[rstest]
#[case::all_true("[1, 2, 3].exists(i, v, i > -1 && v > 0)", 1)]
#[case::some_true("[1, 2, 3].exists(i, v, i == 1 && v == 2)", 1)]
#[case::none_true("![1, 2, 3].exists(i, v, i > 2 && v > 3)", 1)]
#[case::empty("![].exists(i, v, i == 0 || v == 2)", 1)]
// true absorbs errors that come later (short-circuit)
#[case::type_shortcircuit("[1, 'foo', 3].exists(i, v, i == 1 && v != '1')", 1)]
// Map receiver
#[case::map_basic("{'key1':1, 'key2':2}.exists(k, v, k == 'key2' && v == 2)", 1)]
#[case::map_not("!{'key1':1, 'key2':2}.exists(k, v, k == 'key3' || v == 3)", 1)]
fn test_exists_two_var(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ─── all(i, v, pred) ───────────────────────────────────────────────────────

#[rstest]
#[case::all_true("[1, 2, 3].all(i, v, i > -1 && v > 0)", 1)]
#[case::some_true("![1, 2, 3].all(i, v, i == 1 && v == 2)", 1)]
#[case::none_true("![1, 2, 3].all(i, v, i == 3 || v == 4)", 1)]
#[case::empty("[].all(i, v, i > -1 || v > 0)", 1)]
// false absorbs errors that come after (short-circuit)
#[case::error_shortcircuit("[1, 2, 3].all(i, v, 6 / (2 - v) == i) == false", 1)]
// Map receiver
#[case::map_not("!{'key1':1, 'key2':2}.all(k, v, k == 'key2' && v == 2)", 1)]
fn test_all_two_var(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ─── existsOne(i, v, pred) ─────────────────────────────────────────────────

#[rstest]
#[case::empty("![].existsOne(i, v, i == 3 || v == 7)", 1)]
#[case::one_true("[7].existsOne(i, v, i == 0 && v == 7)", 1)]
#[case::one_false("![8].existsOne(i, v, i == 0 && v == 7)", 1)]
#[case::none("![1, 2, 3].existsOne(i, v, i > 2 || v > 3)", 1)]
#[case::one("[5, 7, 8].existsOne(i, v, v % 5 == i)", 1)]
#[case::many("![0, 1, 2, 3, 4].existsOne(i, v, v % 2 == i)", 1)]
// Map receiver — exactly one entry satisfies the predicate
#[case::map_one(
    "{6: 'six', 7: 'seven', 8: 'eight'}.existsOne(k, v, k % 5 == 2 && v == 'seven')",
    1
)]
fn test_exists_one_two_var(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ─── transformList(i, v, expr) ─────────────────────────────────────────────

#[test]
fn test_transform_list_empty() {
    let r = compile_and_execute("[].transformList(i, v, i + v)").unwrap();
    assert_eq!(r, serde_json::json!([]));
}

#[test]
fn test_transform_list_one() {
    let r = compile_and_execute("[3].transformList(i, v, v * v + i)").unwrap();
    assert_eq!(r, serde_json::json!([9]));
}

#[test]
fn test_transform_list_many() {
    let r = compile_and_execute("[2, 4, 6].transformList(i, v, v / 2 + i)").unwrap();
    assert_eq!(r, serde_json::json!([1, 3, 5]));
}

#[test]
fn test_transform_list_filter_empty() {
    let r = compile_and_execute("[].transformList(i, v, i > 0, v)").unwrap();
    assert_eq!(r, serde_json::json!([]));
}

#[test]
fn test_transform_list_filter_one() {
    let r = compile_and_execute("[3].transformList(i, v, i == 0 && v == 3, v * v + i)").unwrap();
    assert_eq!(r, serde_json::json!([9]));
}

#[test]
fn test_transform_list_filter_many() {
    let r =
        compile_and_execute("[2, 4, 6].transformList(i, v, i != 1 && v != 4, v / 2 + i)").unwrap();
    assert_eq!(r, serde_json::json!([1, 5]));
}

// ─── transformMap(k, v, expr) ──────────────────────────────────────────────

#[test]
fn test_transform_map_empty() {
    let r = compile_and_execute("{}.transformMap(k, v, k + v)").unwrap();
    assert_eq!(r, serde_json::json!({}));
}

#[test]
fn test_transform_map_one() {
    let r = compile_and_execute("{'foo': 'bar'}.transformMap(k, v, k + v)").unwrap();
    assert_eq!(r, serde_json::json!({"foo": "foobar"}));
}

#[test]
fn test_transform_map_filter_empty() {
    let r = compile_and_execute("{}.transformMap(k, v, k == 'x', k + v)").unwrap();
    assert_eq!(r, serde_json::json!({}));
}

#[test]
fn test_transform_map_filter_one() {
    let r =
        compile_and_execute("{'foo': 'bar'}.transformMap(k, v, k == 'foo' && v == 'bar', k + v)")
            .unwrap();
    assert_eq!(r, serde_json::json!({"foo": "foobar"}));
}

// transformMap on list receiver: key = index (Int), value = transform result
#[test]
fn test_transform_map_from_list() {
    let r = compile_and_execute(
        "[1, 2, 3].transformMap(indexVar, valueVar, (indexVar * valueVar) + valueVar)",
    )
    .unwrap();
    assert_eq!(r, serde_json::json!({"0": 1, "1": 4, "2": 9}));
}

// ─── transformMapEntry(k, v, entry_expr) ───────────────────────────────────

#[test]
fn test_transform_map_entry_empty_map() {
    let r = compile_and_execute("{}.transformMapEntry(k, v, {v: k})").unwrap();
    assert_eq!(r, serde_json::json!({}));
}

#[test]
fn test_transform_map_entry_key_value_swap() {
    // {'greeting': 'hello'}.transformMapEntry(k, v, {v: k}) → {'hello': 'greeting'}
    let r = compile_and_execute(
        "{'greeting': 'hello'}.transformMapEntry(keyVar, valueVar, {valueVar: keyVar})",
    )
    .unwrap();
    assert_eq!(r, serde_json::json!({"hello": "greeting"}));
}

#[test]
fn test_transform_map_entry_from_list_reverse_index() {
    // [1, 2, 3].transformMapEntry(i, v, {v: i}) → {1: 0, 2: 1, 3: 2}
    // Keys are integers from the list values
    let r = compile_and_execute(
        "[1, 2, 3].transformMapEntry(indexVar, valueVar, {valueVar: indexVar})",
    )
    .unwrap();
    // Int keys serialize as strings in JSON
    assert_eq!(r, serde_json::json!({"1": 0, "2": 1, "3": 2}));
}

#[test]
fn test_transform_map_entry_filter_keep_some() {
    // {'a': 1, 'b': 2, 'c': 3}.transformMapEntry(k, v, v > 1, {k + '_new': v * 10})
    // Only entries where v > 1 are transformed → {'b_new': 20, 'c_new': 30}
    let r = compile_and_execute(
        "{'a': 1, 'b': 2, 'c': 3}.transformMapEntry(k, v, v > 1, {k + '_new': v * 10})",
    )
    .unwrap();
    // Order is unspecified for maps; compare as a set
    let obj = r.as_object().unwrap();
    assert_eq!(obj.len(), 2);
    assert_eq!(obj["b_new"], serde_json::json!(20));
    assert_eq!(obj["c_new"], serde_json::json!(30));
}

#[test]
fn test_transform_map_entry_duplicate_key_error() {
    // {'greeting': 'aloha', 'farewell': 'aloha'}.transformMapEntry(k, v, {v: k})
    // Both entries map to value key 'aloha' → duplicate key error
    let result = compile_and_execute(
        "{'greeting': 'aloha', 'farewell': 'aloha'}.transformMapEntry(k, v, {v: k})",
    )
    .unwrap();
    // Error serializes as {"error": "<message>"}
    let error_msg = result
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        error_msg.contains("insert failed"),
        "Expected duplicate key error, got: {:?}",
        result
    );
}

#[test]
fn test_transform_map_entry_empty_list() {
    let r = compile_and_execute("[].transformMapEntry(i, v, {'k': v})").unwrap();
    assert_eq!(r, serde_json::json!({}));
}
