use crate::common::*;
use rstest::rstest;

// ── regex.replace (all matches) ───────────────────────────────────────────────

#[rstest]
#[case::basic(
    r#"regex.replace("hello world hello", "hello", "bye")"#,
    "bye world bye"
)]
#[case::no_match(r#"regex.replace("hello", r"\d+", "X")"#, "hello")]
#[case::empty_target(r#"regex.replace("", "a", "b")"#, "")]
#[case::pattern_matches_all(r#"regex.replace("aaa", "a", "b")"#, "bbb")]
#[case::capture_group(
    r#"regex.replace("2024-01-15", r"(\d{4})-(\d{2})-(\d{2})", r"\3/\2/\1")"#,
    "15/01/2024"
)]
#[case::literal_backslash_in_repl(r#"regex.replace("abc", "b", r"\\")"#, r"a\c")]
fn test_regex_replace(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("compile_and_execute_string failed");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

// ── regex.replace with count ──────────────────────────────────────────────────

#[rstest]
#[case::count_zero_noop(r#"regex.replace("aaa", "a", "b", 0)"#, "aaa")]
#[case::count_one(r#"regex.replace("aaa", "a", "b", 1)"#, "baa")]
#[case::count_two(r#"regex.replace("aaa", "a", "b", 2)"#, "bba")]
#[case::count_negative_all(r#"regex.replace("aaa", "a", "b", -1)"#, "bbb")]
#[case::count_exceeds_matches(r#"regex.replace("aaa", "a", "b", 100)"#, "bbb")]
fn test_regex_replace_n(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("compile_and_execute_string failed");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

// ── regex.extract ─────────────────────────────────────────────────────────────

#[rstest]
// hasValue() returns true when there is a match
#[case::match_no_group(r#"regex.extract("hello 123 world", r"\d+").hasValue()"#, 1)]
#[case::no_match_no_value(r#"regex.extract("hello world", r"\d+").hasValue()"#, 0)]
#[case::match_with_capture_group_has_value(
    r#"regex.extract("hello 123", r"hello (\d+)").hasValue()"#,
    1
)]
fn test_regex_extract_has_value(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("compile_and_execute failed");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
// .value() returns the matched string
#[case::full_match(r#"regex.extract("hello 123 world", r"\d+").value()"#, "123")]
#[case::capture_group(r#"regex.extract("hello 123 world", r"hello (\d+)").value()"#, "123")]
fn test_regex_extract_value(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("compile_and_execute_string failed");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

// ── regex.extractAll ──────────────────────────────────────────────────────────

#[rstest]
// returns a list — compare size via .size()
#[case::two_matches(r#"regex.extractAll("abc 123 def 456", r"\d+").size()"#, 2)]
#[case::no_matches_empty_list(r#"regex.extractAll("hello world", r"\d+").size()"#, 0)]
#[case::capture_group_two_matches(
    r#"regex.extractAll("key=val1 key=val2", r"key=(\w+)").size()"#,
    2
)]
fn test_regex_extract_all_size(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("compile_and_execute failed");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
// spot-check individual elements
#[case::first_element(r#"regex.extractAll("abc 123 def 456", r"\d+")[0]"#, "123")]
#[case::second_element(r#"regex.extractAll("abc 123 def 456", r"\d+")[1]"#, "456")]
#[case::capture_group_first(r#"regex.extractAll("key=val1 key=val2", r"key=(\w+)")[0]"#, "val1")]
#[case::capture_group_second(r#"regex.extractAll("key=val1 key=val2", r"key=(\w+)")[1]"#, "val2")]
fn test_regex_extract_all_elements(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("compile_and_execute_string failed");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

// ── string.replace must still work (guard correctness) ───────────────────────

#[test]
fn test_string_replace_unaffected() {
    // The regex guard must not capture string.replace()
    let result = compile_and_execute_string(r#""hello world".replace("world", "there")"#)
        .expect("compile_and_execute_string failed");
    assert_eq!(result, "hello there");
}
