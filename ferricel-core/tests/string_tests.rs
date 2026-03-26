// Integration tests for string literals, operations, and built-in string functions.

mod common;
use common::*;

use rstest::rstest;

// ============================================================
// String Tests
// ============================================================

#[rstest]
#[case::basic(r#""hello""#, "hello")]
#[case::empty(r#""""#, "")]
#[case::with_spaces(r#""hello world""#, "hello world")]
#[case::unicode(r#""こんにちは""#, "こんにちは")]
#[case::emoji(r#""hello 👋 world""#, "hello 👋 world")]
#[case::special_chars(r#""!@#$%^&*()""#, "!@#$%^&*()")]
fn test_string_literals(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

#[rstest]
#[case::basic(r#""hello" + " world""#, "hello world")]
#[case::empty_left(r#""" + "test""#, "test")]
#[case::empty_right(r#""test" + """#, "test")]
#[case::both_empty(r#""" + """#, "")]
#[case::unicode(r#""Hello " + "世界""#, "Hello 世界")]
#[case::emoji(r#""Hello " + "👋🌍""#, "Hello 👋🌍")]
#[case::multiple(r#""a" + "b" + "c""#, "abc")]
#[case::with_spaces(r#""hello " + "beautiful " + "world""#, "hello beautiful world")]
fn test_string_concatenation(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

#[rstest]
#[case::basic(r#"size("hello")"#, 5)]
#[case::empty(r#"size("")"#, 0)]
#[case::with_spaces(r#"size("hello world")"#, 11)]
#[case::unicode(r#"size("こんにちは")"#, 5)]
#[case::emoji(r#"size("👋🌍")"#, 2)]
#[case::mixed(r#"size("Hello 世界")"#, 8)]
#[case::concatenation(r#"size("abc" + "def")"#, 6)]
fn test_string_size(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case::basic_true(r#""hello".startsWith("he")"#, 1)]
#[case::basic_false(r#""hello".startsWith("wo")"#, 0)]
#[case::empty_prefix(r#""hello".startsWith("")"#, 1)]
#[case::full_match(r#""hello".startsWith("hello")"#, 1)]
#[case::longer_prefix(r#""hi".startsWith("hello")"#, 0)]
#[case::unicode(r#""こんにちは".startsWith("こん")"#, 1)]
#[case::emoji(r#""👋🌍".startsWith("👋")"#, 1)]
#[case::case_sensitive(r#""Hello".startsWith("hello")"#, 0)]
fn test_string_starts_with(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case::basic_true(r#""hello".endsWith("lo")"#, 1)]
#[case::basic_false(r#""hello".endsWith("he")"#, 0)]
#[case::empty_suffix(r#""hello".endsWith("")"#, 1)]
#[case::full_match(r#""hello".endsWith("hello")"#, 1)]
#[case::longer_suffix(r#""hi".endsWith("hello")"#, 0)]
#[case::unicode(r#""こんにちは".endsWith("ちは")"#, 1)]
#[case::emoji(r#""👋🌍".endsWith("🌍")"#, 1)]
#[case::case_sensitive(r#""Hello".endsWith("HELLO")"#, 0)]
fn test_string_ends_with(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case::basic_true(r#""hello world".contains("lo wo")"#, 1)]
#[case::basic_false(r#""hello".contains("bye")"#, 0)]
#[case::empty_substring(r#""hello".contains("")"#, 1)]
#[case::full_match(r#""hello".contains("hello")"#, 1)]
#[case::at_start(r#""hello".contains("he")"#, 1)]
#[case::at_end(r#""hello".contains("lo")"#, 1)]
#[case::unicode(r#""こんにちは世界".contains("にちは")"#, 1)]
#[case::emoji(r#""Hello 👋 World 🌍".contains("👋")"#, 1)]
#[case::case_sensitive(r#""Hello".contains("hello")"#, 0)]
fn test_string_contains(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case::method_basic_match(r#""foobar".matches("foo.*")"#, 1)]
#[case::method_no_match(r#""hello".matches("world")"#, 0)]
#[case::function_basic_match(r#"matches("foobar", "foo.*")"#, 1)]
#[case::function_no_match(r#"matches("hello", "world")"#, 0)]
#[case::substring_match(r#""hello world".matches("wor")"#, 1)]
#[case::anchored_start_match(r#""foobar".matches("^foo")"#, 1)]
#[case::anchored_start_no_match(r#""foobar".matches("^bar")"#, 0)]
#[case::anchored_end_match(r#""foobar".matches("bar$")"#, 1)]
#[case::anchored_end_no_match(r#""foobar".matches("foo$")"#, 0)]
#[case::full_anchored_match(r#""foobar".matches("^foobar$")"#, 1)]
#[case::full_anchored_no_match(r#""foobar".matches("^foo$")"#, 0)]
#[case::character_class_digit(r#""abc123def".matches("[0-9]+")"#, 1)]
#[case::character_class_letter(r#""abc123def".matches("[a-z]+")"#, 1)]
#[case::quantifier_plus(r#""aaaa".matches("a+")"#, 1)]
#[case::quantifier_star(r#""".matches("a*")"#, 1)]
#[case::quantifier_question(r#""colour".matches("colou?r")"#, 1)]
#[case::quantifier_exact(r#""aaaa".matches("a{4}")"#, 1)]
#[case::quantifier_range(r#""aaaa".matches("a{3,5}")"#, 1)]
#[case::dot_wildcard(r#""a_b".matches("a.b")"#, 1)]
#[case::alternation(r#""cat".matches("cat|dog")"#, 1)]
#[case::unicode_pattern(r#""Hello 世界".matches("世界")"#, 1)]
#[case::emoji_pattern(r#""Hello 😀 World".matches("😀")"#, 1)]
#[case::email_pattern(r#""test@example.com".matches("[a-z]+@[a-z]+\\.[a-z]+")"#, 1)]
#[case::case_sensitive(r#""Hello".matches("hello")"#, 0)]
#[case::case_insensitive_flag(r#""Hello".matches("(?i)hello")"#, 1)]
fn test_string_matches(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}
