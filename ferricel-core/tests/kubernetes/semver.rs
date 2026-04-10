//! Integration tests for the Kubernetes CEL semver library.
//!
//! Tests are ported from the Go implementation in:
//!   k8s.io/apiserver/pkg/cel/library/semver_test.go

use crate::common::*;
use rstest::rstest;

// ── isSemver(string) ─────────────────────────────────────────────────────────

#[rstest]
// Valid strict semver
#[case(r#"isSemver("1.2.3")"#, 1)]
#[case(r#"isSemver("1.2.3-alpha")"#, 1)]
#[case(r#"isSemver("1.2.3+build")"#, 1)]
#[case(r#"isSemver("1.2.3-alpha+build")"#, 1)]
#[case(r#"isSemver("0.0.0")"#, 1)]
// Invalid strict semver
#[case(r#"isSemver("v1.2.3")"#, 0)] // v prefix
#[case(r#"isSemver("1.2")"#, 0)] // missing patch
#[case(r#"isSemver("1")"#, 0)] // only major
#[case(r#"isSemver("not-a-semver")"#, 0)]
#[case(r#"isSemver("")"#, 0)]
#[case(r#"isSemver("01.02.03")"#, 0)] // leading zeros not allowed in strict
fn test_is_semver(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── isSemver(string, bool) ───────────────────────────────────────────────────

#[rstest]
// Normalize=false (same as isSemver(str))
#[case(r#"isSemver("1.2.3", false)"#, 1)]
#[case(r#"isSemver("v1.2.3", false)"#, 0)]
#[case(r#"isSemver("1.2", false)"#, 0)]
// Normalize=true
#[case(r#"isSemver("v1.2.3", true)"#, 1)]
#[case(r#"isSemver("1.2", true)"#, 1)]
#[case(r#"isSemver("1", true)"#, 1)]
#[case(r#"isSemver("01.02.03", true)"#, 1)] // leading zeros stripped
#[case(r#"isSemver("v01.01", true)"#, 1)] // v prefix + short + leading zeros
#[case(r#"isSemver("1.0.0-alpha", true)"#, 1)] // pre-release allowed at 3 parts
#[case(r#"isSemver("1.0.0+build", true)"#, 1)] // build meta allowed at 3 parts
#[case(r#"isSemver("1-alpha", true)"#, 0)] // short version with pre-release
#[case(r#"isSemver("1+build", true)"#, 0)] // short version with build meta
#[case(r#"isSemver("1.0-alpha", true)"#, 0)] // 2-part with pre-release
#[case(r#"isSemver("not-a-semver", true)"#, 0)]
fn test_is_semver_normalize(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── semver(string) — successful parse ────────────────────────────────────────

#[test]
fn test_semver_parse_returns_semver() {
    // semver() returns a Semver, which serialises to a string
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
    // From Go test: equality_normalize: semver("v01.01", true) == semver("1.1.0")
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

// major/minor/patch from normalized
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
#[case(r#"semver("1.0.0").isLessThan(semver("2.0.0"))"#, 1)]
#[case(r#"semver("1.0.0").isLessThan(semver("1.1.0"))"#, 1)]
#[case(r#"semver("1.0.0").isLessThan(semver("1.0.1"))"#, 1)]
#[case(r#"semver("1.0.0-alpha").isLessThan(semver("1.0.0"))"#, 1)] // pre-release < release
#[case(r#"semver("1.0.0").isLessThan(semver("1.0.0"))"#, 0)]
#[case(r#"semver("2.0.0").isLessThan(semver("1.0.0"))"#, 0)]
fn test_is_less_than(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── isGreaterThan() ──────────────────────────────────────────────────────────

#[rstest]
#[case(r#"semver("2.0.0").isGreaterThan(semver("1.0.0"))"#, 1)]
#[case(r#"semver("1.1.0").isGreaterThan(semver("1.0.0"))"#, 1)]
#[case(r#"semver("1.0.1").isGreaterThan(semver("1.0.0"))"#, 1)]
#[case(r#"semver("1.0.0").isGreaterThan(semver("1.0.0-alpha"))"#, 1)] // release > pre-release
#[case(r#"semver("1.0.0").isGreaterThan(semver("1.0.0"))"#, 0)]
#[case(r#"semver("1.0.0").isGreaterThan(semver("2.0.0"))"#, 0)]
fn test_is_greater_than(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
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
// compareTo ignores build metadata (uses cmp_precedence)
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
// reflexivity
#[case(r#"semver("1.2.3") == semver("1.2.3")"#, 1)]
#[case(r#"semver("1.2.3-alpha+build") == semver("1.2.3-alpha+build")"#, 1)]
// normalize equality: semver("v01.01", true) == semver("1.1.0")
#[case(r#"semver("v01.01", true) == semver("1.1.0")"#, 1)]
// inequality
#[case(r#"semver("1.2.3") == semver("1.2.4")"#, 0)]
#[case(r#"semver("1.2.3") == semver("1.3.0")"#, 0)]
fn test_equality(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
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

// ── whitespace not trimmed (unlike ParseTolerant) ─────────────────────────────

#[test]
fn test_semver_whitespace_rejected() {
    let result =
        compile_and_execute(r#"isSemver(" 1.0.0")"#).expect("Failed to compile and execute");
    assert_eq!(result, 0, "whitespace should not be trimmed");
}

#[test]
fn test_semver_whitespace_normalize_rejected() {
    let result =
        compile_and_execute(r#"isSemver(" 1.0.0", true)"#).expect("Failed to compile and execute");
    assert_eq!(
        result, 0,
        "whitespace should not be trimmed during normalize"
    );
}
