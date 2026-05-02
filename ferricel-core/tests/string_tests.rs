// Integration tests for string literals, operations, and built-in string functions.

use crate::common::*;

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
#[case::basic_true(r#""hello".startsWith("he")"#, true)]
#[case::basic_false(r#""hello".startsWith("wo")"#, false)]
#[case::empty_prefix(r#""hello".startsWith("")"#, true)]
#[case::full_match(r#""hello".startsWith("hello")"#, true)]
#[case::longer_prefix(r#""hi".startsWith("hello")"#, false)]
#[case::unicode(r#""こんにちは".startsWith("こん")"#, true)]
#[case::emoji(r#""👋🌍".startsWith("👋")"#, true)]
#[case::case_sensitive(r#""Hello".startsWith("hello")"#, false)]
fn test_string_starts_with(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case::basic_true(r#""hello".endsWith("lo")"#, true)]
#[case::basic_false(r#""hello".endsWith("he")"#, false)]
#[case::empty_suffix(r#""hello".endsWith("")"#, true)]
#[case::full_match(r#""hello".endsWith("hello")"#, true)]
#[case::longer_suffix(r#""hi".endsWith("hello")"#, false)]
#[case::unicode(r#""こんにちは".endsWith("ちは")"#, true)]
#[case::emoji(r#""👋🌍".endsWith("🌍")"#, true)]
#[case::case_sensitive(r#""Hello".endsWith("HELLO")"#, false)]
fn test_string_ends_with(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case::basic_true(r#""hello world".contains("lo wo")"#, true)]
#[case::basic_false(r#""hello".contains("bye")"#, false)]
#[case::empty_substring(r#""hello".contains("")"#, true)]
#[case::full_match(r#""hello".contains("hello")"#, true)]
#[case::at_start(r#""hello".contains("he")"#, true)]
#[case::at_end(r#""hello".contains("lo")"#, true)]
#[case::unicode(r#""こんにちは世界".contains("にちは")"#, true)]
#[case::emoji(r#""Hello 👋 World 🌍".contains("👋")"#, true)]
#[case::case_sensitive(r#""Hello".contains("hello")"#, false)]
fn test_string_contains(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case::method_basic_match(r#""foobar".matches("foo.*")"#, true)]
#[case::method_no_match(r#""hello".matches("world")"#, false)]
#[case::function_basic_match(r#"matches("foobar", "foo.*")"#, true)]
#[case::function_no_match(r#"matches("hello", "world")"#, false)]
#[case::substring_match(r#""hello world".matches("wor")"#, true)]
#[case::anchored_start_match(r#""foobar".matches("^foo")"#, true)]
#[case::anchored_start_no_match(r#""foobar".matches("^bar")"#, false)]
#[case::anchored_end_match(r#""foobar".matches("bar$")"#, true)]
#[case::anchored_end_no_match(r#""foobar".matches("foo$")"#, false)]
#[case::full_anchored_match(r#""foobar".matches("^foobar$")"#, true)]
#[case::full_anchored_no_match(r#""foobar".matches("^foo$")"#, false)]
#[case::character_class_digit(r#""abc123def".matches("[0-9]+")"#, true)]
#[case::character_class_letter(r#""abc123def".matches("[a-z]+")"#, true)]
#[case::quantifier_plus(r#""aaaa".matches("a+")"#, true)]
#[case::quantifier_star(r#""".matches("a*")"#, true)]
#[case::quantifier_question(r#""colour".matches("colou?r")"#, true)]
#[case::quantifier_exact(r#""aaaa".matches("a{4}")"#, true)]
#[case::quantifier_range(r#""aaaa".matches("a{3,5}")"#, true)]
#[case::dot_wildcard(r#""a_b".matches("a.b")"#, true)]
#[case::alternation(r#""cat".matches("cat|dog")"#, true)]
#[case::unicode_pattern(r#""Hello 世界".matches("世界")"#, true)]
#[case::emoji_pattern(r#""Hello 😀 World".matches("😀")"#, true)]
#[case::email_pattern(r#""test@example.com".matches("[a-z]+@[a-z]+\\.[a-z]+")"#, true)]
#[case::case_sensitive(r#""Hello".matches("hello")"#, false)]
#[case::case_insensitive_flag(r#""Hello".matches("(?i)hello")"#, true)]
fn test_string_matches(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}
