use crate::common::*;
use rstest::rstest;

// ── join ──────────────────────────────────────────────────────────────────────

#[rstest]
#[case::no_sep(r#"["a", "b", "c"].join()"#, "abc")]
#[case::with_sep(r#"["a", "b", "c"].join(",")"#, "a,b,c")]
#[case::empty_list(r#"[].join(",")"#, "")]
fn test_join(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

// ── distinct ──────────────────────────────────────────────────────────────────

#[rstest]
#[case::empty("[].distinct() == []", 1)]
#[case::single("[1].distinct() == [1]", 1)]
#[case::ints("[-2, 5, -2, 1, 1, 5, -2, 1].distinct() == [-2, 5, 1]", 1)]
#[case::strings(
    r#"['c', 'a', 'a', 'b', 'a', 'b', 'c', 'c'].distinct() == ['c', 'a', 'b']"#,
    1
)]
#[case::mixed_types(r#"[1, 2.0, "c", 3, "c", 1].distinct() == [1, 2.0, "c", 3]"#, 1)]
#[case::cross_type_numeric("[1, 1.0, 2].distinct() == [1, 2]", 1)]
#[case::nested_lists("[[1], [1], [2]].distinct() == [[1], [2]]", 1)]
fn test_distinct(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ── flatten ───────────────────────────────────────────────────────────────────

#[rstest]
#[case::empty("dyn([]).flatten() == []", 1)]
#[case::already_flat("dyn([1,2,3,4]).flatten() == [1,2,3,4]", 1)]
#[case::one_level("[1,[2,[3,4]]].flatten() == [1,2,[3,4]]", 1)]
#[case::empty_sublists("[1,2,[],[],[3,4]].flatten() == [1,2,3,4]", 1)]
#[case::depth_2("[1,[2,[3,4]]].flatten(2) == [1,2,3,4]", 1)]
#[case::depth_deep("[1,[2,[3,[4]]]].flatten(2) == [1,2,3,[4]]", 1)]
fn test_flatten(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[test]
fn test_flatten_negative_depth_returns_error() {
    let result = compile_and_execute("[].flatten(-1)");
    assert!(result.is_err(), "Expected error for negative flatten depth");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("non-negative"),
        "Expected 'non-negative' in error, got: {}",
        msg
    );
}

// ── lists.range ───────────────────────────────────────────────────────────────

#[rstest]
#[case::empty("lists.range(0) == []", 1)]
#[case::four("lists.range(4) == [0,1,2,3]", 1)]
fn test_range(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ── reverse ───────────────────────────────────────────────────────────────────

#[rstest]
// list reverse
#[case::list_empty("[].reverse() == []", 1)]
#[case::list_single("[1].reverse() == [1]", 1)]
#[case::list_ints("[5,1,2,3].reverse() == [3,2,1,5]", 1)]
#[case::list_strings(r#"['are','you','as','bored','as','I','am'].reverse() == ['am','I','as','bored','as','you','are']"#, 1)]
#[case::list_double_reverse("[false, true, true].reverse().reverse() == [false, true, true]", 1)]
// string reverse (polymorphic — same function name)
#[case::string_basic(r#""gums".reverse() == "smug""#, 1)]
#[case::string_empty(r#""".reverse() == """#, 1)]
fn test_reverse(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ── slice ─────────────────────────────────────────────────────────────────────

#[rstest]
#[case::full("[1,2,3,4].slice(0, 4) == [1,2,3,4]", 1)]
#[case::empty_start("[1,2,3,4].slice(0, 0) == []", 1)]
#[case::empty_mid("[1,2,3,4].slice(1, 1) == []", 1)]
#[case::empty_end("[1,2,3,4].slice(4, 4) == []", 1)]
#[case::middle("[1,2,3,4].slice(1, 3) == [2, 3]", 1)]
#[case::tail("[1,2,3,4].slice(2, 4) == [3, 4]", 1)]
fn test_slice(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case::start_after_end(
    "[1,2,3,4].slice(3, 0)",
    "start index must be less than or equal to end index"
)]
#[case::end_out_of_bounds("[1,2,3,4].slice(0, 10)", "list is length 4")]
#[case::negative_start("[1,2,3,4].slice(-5, 10)", "negative indexes not supported")]
#[case::both_negative("[1,2,3,4].slice(-5, -3)", "negative indexes not supported")]
fn test_slice_errors(#[case] expr: &str, #[case] msg: &str) {
    let result = compile_and_execute(expr);
    assert!(result.is_err(), "Expected error for '{}'", expr);
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains(msg),
        "Expected '{}' in error, got: {}",
        msg,
        err
    );
}

// ── sort ──────────────────────────────────────────────────────────────────────

#[rstest]
#[case::empty("[].sort() == []", 1)]
#[case::single("[1].sort() == [1]", 1)]
#[case::ints("[4, 3, 2, 1].sort() == [1, 2, 3, 4]", 1)]
#[case::strings(r#"["d", "a", "b", "c"].sort() == ["a", "b", "c", "d"]"#, 1)]
fn test_sort(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[test]
fn test_sort_mixed_types_returns_error() {
    let result = compile_and_execute(r#"["d", 3, 2, "c"].sort()"#);
    assert!(result.is_err(), "Expected error for mixed-type sort");
}

// ── first ─────────────────────────────────────────────────────────────────────

#[rstest]
// empty list → optional.none() → hasValue() == false
#[case::empty_has_value("[].first().hasValue()", 0)]
// orValue fallback on empty
#[case::empty_or_value("[].first().orValue(99) == 99", 1)]
// ints
#[case::ints_value("[1, 2, 3].first().value() == 1", 1)]
#[case::ints_has_value("[1, 2, 3].first().hasValue()", 1)]
// strings
#[case::strings_value(r#"["a", "b", "c"].first().value() == "a""#, 1)]
#[case::strings_or_value(r#"["z"].first().orValue("fallback") == "z""#, 1)]
// bools
#[case::bools_value("[true, false].first().value() == true", 1)]
// doubles
#[case::doubles_value("[1.5, 2.5].first().value() == 1.5", 1)]
// single element
#[case::single("[42].first().value() == 42", 1)]
fn test_first(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ── last ──────────────────────────────────────────────────────────────────────

#[rstest]
// empty list → optional.none() → hasValue() == false
#[case::empty_has_value("[].last().hasValue()", 0)]
// orValue fallback on empty
#[case::empty_or_value(r#"[].last().orValue("test") == "test""#, 1)]
// ints
#[case::ints_value("[1, 2, 3].last().value() == 3", 1)]
#[case::ints_has_value("[1, 2, 3].last().hasValue()", 1)]
// strings
#[case::strings_value(r#"["a", "b", "c"].last().value() == "c""#, 1)]
#[case::strings_or_value(r#"["z"].last().orValue("fallback") == "z""#, 1)]
// bools
#[case::bools_value("[true, false].last().value() == false", 1)]
// doubles
#[case::doubles_value("[1.5, 2.5].last().value() == 2.5", 1)]
// single element — first and last are the same
#[case::single("[42].last().value() == 42", 1)]
// first and last differ on a multi-element list
#[case::first_ne_last("[1, 2, 3].first().value() != [1, 2, 3].last().value()", 1)]
fn test_last(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

// ── sortBy ────────────────────────────────────────────────────────────────────
// Test cases ported from google/cel-go ext/lists_test.go (TestLists + TestListsCosts)

#[rstest]
// empty list (from cel-go: `[].sortBy(e, e) == []`)
#[case::empty("[].sortBy(e, e) == []", 1)]
// single element (from cel-go: `["a"].sortBy(e, e) == ["a"]`)
#[case::single(r#"["a"].sortBy(e, e) == ["a"]"#, 1)]
// int key transform: sort by negated square (from cel-go)
#[case::int_key_transform("[-3, 1, -5, -2, 4].sortBy(e, -(e * e)) == [-5, 4, -3, -2, 1]", 1)]
// chained with map (from cel-go)
#[case::chained_map(
    "[-3, 1, -5, -2, 4].map(e, e * 2).sortBy(e, -(e * e)) == [-10, 8, -6, -4, 2]",
    1
)]
// chained with lists.range (from cel-go)
#[case::chained_range("lists.range(3).sortBy(e, -e) == [2, 1, 0]", 1)]
// conditional key expression (from cel-go)
#[case::conditional_key(
    r#"["a", "c", "b", "first"].sortBy(e, e == "first" ? "" : e) == ["first", "a", "b", "c"]"#,
    1
)]
// sort maps by int field (from cel-go TestListsCosts list_sortBy)
#[case::maps_int_field(r#"[{"x": 4}, {"x": 3}].sortBy(m, m["x"]) == [{"x": 3}, {"x": 4}]"#, 1)]
// sort maps by string field (from cel-go TestListsVersion version=2)
#[case::maps_string_field(
    r#"[{"field": "lo"}, {"field": "hi"}].sortBy(m, m["field"]) == [{"field": "hi"}, {"field": "lo"}]"#,
    1
)]
// plain string sort (identity key)
#[case::strings(r#"["d", "b", "c", "a"].sortBy(e, e) == ["a", "b", "c", "d"]"#, 1)]
// plain int sort (identity key)
#[case::ints("[3, 1, 4, 1, 5, 9, 2, 6].sortBy(e, e) == [1, 1, 2, 3, 4, 5, 6, 9]", 1)]
fn test_sort_by(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}
