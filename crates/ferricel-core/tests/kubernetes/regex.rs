use rstest::rstest;

use crate::common::*;

// ── find ────────────────────────────────────────────────────────────────────

#[rstest]
#[case(r#""abc 123".find("[0-9]+")"#, "123")]
#[case(r#""abc 123".find("xyz")"#, "")]
#[case(r#""".find("[0-9]+")"#, "")]
#[case(r#""hello world".find("^hello")"#, "hello")]
#[case(r#""say hello".find("^hello")"#, "")]
fn test_k8s_regex_find(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {:?}",
        expr, expected
    );
}

#[test]
fn test_k8s_regex_find_first_match_only() {
    // find() returns only the first match, not all of them
    let result = compile_and_execute_string(r#""123 abc 456".find("[0-9]+")"#)
        .expect("compile/execute failed");
    assert_eq!(result, "123");
}

// ── findAll (no limit) ───────────────────────────────────────────────────────

#[test]
fn test_k8s_regex_find_all_multiple_matches() {
    let result =
        compile_and_execute(r#""123 abc 456".findAll("[0-9]+")"#).expect("compile/execute failed");
    assert_eq!(
        result,
        serde_json::json!(["123", "456"]),
        "findAll should return all matches"
    );
}

#[test]
fn test_k8s_regex_find_all_no_matches() {
    let result =
        compile_and_execute(r#""abc def".findAll("[0-9]+")"#).expect("compile/execute failed");
    assert_eq!(
        result,
        serde_json::json!([]),
        "findAll should return empty list on no match"
    );
}

#[test]
fn test_k8s_regex_find_all_single_match() {
    let result = compile_and_execute(r#""only 1 number".findAll("[0-9]+")"#)
        .expect("compile/execute failed");
    assert_eq!(result, serde_json::json!(["1"]));
}

#[test]
fn test_k8s_regex_find_all_empty_string() {
    let result = compile_and_execute(r#""".findAll("[0-9]+")"#).expect("compile/execute failed");
    assert_eq!(result, serde_json::json!([]));
}

// ── findAll with limit ───────────────────────────────────────────────────────

#[test]
fn test_k8s_regex_find_all_n_limit_one() {
    let result = compile_and_execute(r#""123 abc 456 def 789".findAll("[0-9]+", 1)"#)
        .expect("compile/execute failed");
    assert_eq!(result, serde_json::json!(["123"]));
}

#[test]
fn test_k8s_regex_find_all_n_limit_two() {
    let result = compile_and_execute(r#""123 abc 456 def 789".findAll("[0-9]+", 2)"#)
        .expect("compile/execute failed");
    assert_eq!(result, serde_json::json!(["123", "456"]));
}

#[test]
fn test_k8s_regex_find_all_n_limit_exceeds_matches() {
    let result = compile_and_execute(r#""123 abc 456".findAll("[0-9]+", 10)"#)
        .expect("compile/execute failed");
    assert_eq!(result, serde_json::json!(["123", "456"]));
}

#[test]
fn test_k8s_regex_find_all_n_limit_zero() {
    let result = compile_and_execute(r#""123 abc 456".findAll("[0-9]+", 0)"#)
        .expect("compile/execute failed");
    assert_eq!(result, serde_json::json!([]));
}
