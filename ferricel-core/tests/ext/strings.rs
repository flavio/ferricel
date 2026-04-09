use crate::common::*;
use rstest::rstest;

#[rstest]
#[case::basic(r#""hello".charAt(1)"#, "e")]
#[case::first(r#""hello".charAt(0)"#, "h")]
#[case::last(r#""hello".charAt(4)"#, "o")]
#[case::end_sentinel(r#""hello".charAt(5)"#, "")]
#[case::unicode(r#""café".charAt(2)"#, "f")]
fn test_char_at(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

#[rstest]
#[case::basic(r#""Hello World".lowerAscii()"#, "hello world")]
#[case::already_lower(r#""hello".lowerAscii()"#, "hello")]
#[case::empty(r#""".lowerAscii()"#, "")]
fn test_lower_ascii(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

#[rstest]
#[case::basic(r#""hello world".upperAscii()"#, "HELLO WORLD")]
#[case::already_upper(r#""HELLO".upperAscii()"#, "HELLO")]
#[case::empty(r#""".upperAscii()"#, "")]
fn test_upper_ascii(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

#[rstest]
#[case::basic(r#""hello hello".replace("he", "we")"#, "wello wello")]
#[case::no_match(r#""hello".replace("xyz", "abc")"#, "hello")]
#[case::with_count(r#""hello hello hello".replace("hello", "bye", 1)"#, "bye hello hello")]
fn test_replace(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

#[rstest]
#[case::basic(r#""a,b,c".split(",")"#, r#"["a","b","c"]"#)]
#[case::single_part(r#""abc".split("x")"#, r#"["abc"]"#)]
#[case::with_limit(r#""a,b,c".split(",", 2)"#, r#"["a","b,c"]"#)]
fn test_split(#[case] expr: &str, #[case] expected_json: &str) {
    let result = compile_and_execute_json(expr).expect("Failed to compile and execute");
    let expected: serde_json::Value = serde_json::from_str(expected_json).unwrap();
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected_json
    );
}

#[rstest]
#[case::basic(r#""hello world".substring(6)"#, "world")]
#[case::from_start(r#""hello".substring(0)"#, "hello")]
#[case::range(r#""hello world".substring(6, 11)"#, "world")]
#[case::empty_range(r#""hello".substring(2, 2)"#, "")]
fn test_substring(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

#[rstest]
#[case::basic(r#""  hello  ".trim()"#, "hello")]
#[case::no_whitespace(r#""hello".trim()"#, "hello")]
#[case::empty(r#""".trim()"#, "")]
fn test_trim(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

#[rstest]
#[case::basic(r#""hello".reverse()"#, "olleh")]
#[case::palindrome(r#""racecar".reverse()"#, "racecar")]
#[case::empty(r#""".reverse()"#, "")]
#[case::unicode(r#""café".reverse()"#, "éfac")]
fn test_reverse(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

#[rstest]
#[case::basic(r#"strings.quote("hello")"#, r#""hello""#)]
#[case::with_tab(r#"strings.quote("a\tb")"#, r#""a\tb""#)]
fn test_strings_quote(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}

#[rstest]
#[case::string_verb(r#""%s world".format(["hello"])"#, "hello world")]
#[case::int_verb(r#""value: %d".format([42])"#, "value: 42")]
#[case::float_verb(r#""pi: %.2f".format([3.14159])"#, "pi: 3.14")]
#[case::hex_verb(r#""%x".format([255])"#, "ff")]
#[case::percent_escape(r#""100%%".format([])"#, "100%")]
fn test_format(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}
