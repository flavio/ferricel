//! Integration tests for the Kubernetes CEL semver library.

use crate::common::*;
use rstest::rstest;

// ── isSemver(string) ─────────────────────────────────────────────────────────

#[rstest]
#[case(r#"isSemver("1.2.3")"#, true)]
#[case(r#"isSemver("1.2.3-alpha")"#, true)]
#[case(r#"isSemver("1.2.3+build")"#, true)]
#[case(r#"isSemver("1.2.3-alpha+build")"#, true)]
#[case(r#"isSemver("0.0.0")"#, true)]
#[case(r#"isSemver("v1.2.3")"#, false)] // v prefix
#[case(r#"isSemver("1.2")"#, false)] // missing patch
#[case(r#"isSemver("1")"#, false)] // only major
#[case(r#"isSemver("not-a-semver")"#, false)]
#[case(r#"isSemver("")"#, false)]
#[case(r#"isSemver("01.02.03")"#, false)] // leading zeros not allowed in strict
fn test_is_semver(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── isSemver(string, bool) ───────────────────────────────────────────────────

#[rstest]
#[case(r#"isSemver("1.2.3", false)"#, true)]
#[case(r#"isSemver("v1.2.3", false)"#, false)]
#[case(r#"isSemver("1.2", false)"#, false)]
#[case(r#"isSemver("v1.2.3", true)"#, true)]
#[case(r#"isSemver("1.2", true)"#, true)]
#[case(r#"isSemver("1", true)"#, true)]
#[case(r#"isSemver("01.02.03", true)"#, true)]
#[case(r#"isSemver("v01.01", true)"#, true)]
#[case(r#"isSemver("1.0.0-alpha", true)"#, true)]
#[case(r#"isSemver("1.0.0+build", true)"#, true)]
#[case(r#"isSemver("1-alpha", true)"#, false)]
#[case(r#"isSemver("1+build", true)"#, false)]
#[case(r#"isSemver("1.0-alpha", true)"#, false)]
#[case(r#"isSemver("not-a-semver", true)"#, false)]
fn test_is_semver_normalize(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── semver(string) — successful parse ────────────────────────────────────────

#[test]
fn test_semver_parse_returns_semver() {
    let result = compile_and_execute_string(r#"string(semver("1.2.3"))"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, "1.2.3");
}

#[test]
fn test_semver_parse_with_prerelease() {
    let result = compile_and_execute_string(r#"string(semver("1.0.0-alpha+build"))"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, "1.0.0-alpha+build");
}

// ── semver(string) — error on invalid ────────────────────────────────────────

#[test]
fn test_semver_parse_invalid_returns_error() {
    let result = compile_and_execute(r#"semver("v1.2.3").major()"#);
    assert!(
        result.is_err(),
        "Expected error for semver(\"v1.2.3\"), got {:?}",
        result
    );
}

// ── semver(string, bool) — normalize ─────────────────────────────────────────

#[test]
fn test_semver_parse_normalize_v_prefix() {
    let result = compile_and_execute_string(r#"string(semver("v1.2.3", true))"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, "1.2.3");
}

#[test]
fn test_semver_parse_normalize_short() {
    let result = compile_and_execute_string(r#"string(semver("1.2", true))"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, "1.2.0");
}

#[test]
fn test_semver_parse_normalize_leading_zeros() {
    let result = compile_and_execute_string(r#"string(semver("01.01", true))"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, "1.1.0");
}

#[test]
fn test_semver_parse_normalize_v_and_leading_zeros() {
    let result = compile_and_execute_string(r#"string(semver("v01.01", true))"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, "1.1.0");
}

// ── major() / minor() / patch() ──────────────────────────────────────────────

#[rstest]
#[case(r#"semver("1.2.3").major()"#, 1)]
#[case(r#"semver("0.5.9").major()"#, 0)]
#[case(r#"semver("10.0.0").major()"#, 10)]
fn test_major(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

#[rstest]
#[case(r#"semver("1.2.3").minor()"#, 2)]
#[case(r#"semver("0.5.9").minor()"#, 5)]
#[case(r#"semver("10.0.0").minor()"#, 0)]
fn test_minor(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

#[rstest]
#[case(r#"semver("1.2.3").patch()"#, 3)]
#[case(r#"semver("0.5.9").patch()"#, 9)]
#[case(r#"semver("10.0.0").patch()"#, 0)]
fn test_patch(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

#[test]
fn test_major_from_normalized() {
    let result = compile_and_execute(r#"semver("v3.5.1", true).major()"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, 3);
}

#[test]
fn test_minor_from_normalized() {
    let result = compile_and_execute(r#"semver("v3.5.1", true).minor()"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, 5);
}

#[test]
fn test_patch_from_normalized() {
    let result = compile_and_execute(r#"semver("v3.5.1", true).patch()"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, 1);
}

// ── isLessThan() ─────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"semver("1.0.0").isLessThan(semver("2.0.0"))"#, true)]
#[case(r#"semver("1.0.0").isLessThan(semver("1.1.0"))"#, true)]
#[case(r#"semver("1.0.0").isLessThan(semver("1.0.1"))"#, true)]
#[case(r#"semver("1.0.0-alpha").isLessThan(semver("1.0.0"))"#, true)]
#[case(r#"semver("1.0.0").isLessThan(semver("1.0.0"))"#, false)]
#[case(r#"semver("2.0.0").isLessThan(semver("1.0.0"))"#, false)]
fn test_is_less_than(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── isGreaterThan() ──────────────────────────────────────────────────────────

#[rstest]
#[case(r#"semver("2.0.0").isGreaterThan(semver("1.0.0"))"#, true)]
#[case(r#"semver("1.1.0").isGreaterThan(semver("1.0.0"))"#, true)]
#[case(r#"semver("1.0.1").isGreaterThan(semver("1.0.0"))"#, true)]
#[case(r#"semver("1.0.0").isGreaterThan(semver("1.0.0-alpha"))"#, true)]
#[case(r#"semver("1.0.0").isGreaterThan(semver("1.0.0"))"#, false)]
#[case(r#"semver("1.0.0").isGreaterThan(semver("2.0.0"))"#, false)]
fn test_is_greater_than(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── compareTo() ──────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"semver("1.0.0").compareTo(semver("2.0.0"))"#, -1)]
#[case(r#"semver("2.0.0").compareTo(semver("1.0.0"))"#, 1)]
#[case(r#"semver("1.0.0").compareTo(semver("1.0.0"))"#, 0)]
#[case(r#"semver("1.0.0+build.1").compareTo(semver("1.0.0+build.2"))"#, 0)]
fn test_compare_to(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── equality ─────────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"semver("1.2.3") == semver("1.2.3")"#, true)]
#[case(r#"semver("1.2.3-alpha+build") == semver("1.2.3-alpha+build")"#, true)]
#[case(r#"semver("v01.01", true) == semver("1.1.0")"#, true)]
#[case(r#"semver("1.2.3") == semver("1.2.4")"#, false)]
#[case(r#"semver("1.2.3") == semver("1.3.0")"#, false)]
fn test_equality(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── semver serializes to canonical string ─────────────────────────────────────

#[test]
fn test_semver_serializes_to_string() {
    let result = compile_and_execute_string(r#"string(semver("1.2.3"))"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, "1.2.3");
}

#[test]
fn test_semver_with_prerelease_serializes() {
    let result = compile_and_execute_string(r#"string(semver("1.0.0-alpha"))"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, "1.0.0-alpha");
}

// ── whitespace not trimmed ─────────────────────────────────────────────────────

#[test]
fn test_semver_whitespace_rejected() {
    let result =
        compile_and_execute_bool(r#"isSemver(" 1.0.0")"#).expect("Failed to compile and execute");
    assert!(!result, "whitespace should not be trimmed");
}

#[test]
fn test_semver_whitespace_normalize_rejected() {
    let result = compile_and_execute_bool(r#"isSemver(" 1.0.0", true)"#)
        .expect("Failed to compile and execute");
    assert!(!result, "whitespace should not be trimmed during normalize");
}

// ── cross-type dispatch errors ───────────────────────────────────────────────

#[rstest]
#[case(r#"semver("1.0.0").isLessThan(quantity("1k"))"#)]
#[case(r#"semver("1.0.0").isGreaterThan(quantity("1k"))"#)]
#[case(r#"semver("1.0.0").compareTo(quantity("1k"))"#)]
fn test_semver_cross_type_dispatch_error(#[case] expr: &str) {
    let result = compile_and_execute(expr);
    assert!(
        result.is_err(),
        "Expected error for '{}', got {:?}",
        expr,
        result
    );
}
