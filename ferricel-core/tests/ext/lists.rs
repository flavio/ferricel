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
