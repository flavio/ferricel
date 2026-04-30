//! Kubernetes CEL regex library extensions.
//!
//! Implements the additional string methods that Kubernetes adds to CEL:
//!   - `find`    — returns the first substring matching a regex, or "" if no match
//!   - `findAll` — returns all substrings matching a regex, up to an optional limit
//!     (`findAll(pattern, -1)` returns all matches)
//!
//! Reference: <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-regex-library>

use crate::error::{create_error_value, null_to_unbound};
use crate::types::CelValue;
use regex_lite::Regex;
use slog::error;

// ──────────────────────────────────────────────────────────────────────────────
// find
// ──────────────────────────────────────────────────────────────────────────────

/// Returns the first substring of `string_ptr` that matches the regex in
/// `pattern_ptr`, or an empty string if there is no match.
///
/// Returns an error value if either argument is not a String or the regex is
/// invalid.
///
/// # Safety
/// Both `string_ptr` and `pattern_ptr` must be valid, non-null pointers to
/// `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_regex_find(
    string_ptr: *mut CelValue,
    pattern_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if string_ptr.is_null() || pattern_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_regex_find");
        return create_error_value("no such overload");
    }

    let string_val = unsafe { null_to_unbound(string_ptr) };
    let pattern_val = unsafe { null_to_unbound(pattern_ptr) };

    let (s, pattern) = match (&string_val, &pattern_val) {
        (CelValue::String(s), CelValue::String(p)) => (s.clone(), p.clone()),
        _ => {
            error!(log, "expected String arguments";
                "function" => "cel_k8s_regex_find",
                "string" => format!("{:?}", string_val),
                "pattern" => format!("{:?}", pattern_val));
            return create_error_value("no such overload");
        }
    };

    let re = match Regex::new(&pattern) {
        Ok(r) => r,
        Err(e) => {
            error!(log, "invalid regex";
                "function" => "cel_k8s_regex_find",
                "pattern" => &pattern,
                "error" => format!("{}", e));
            return create_error_value("invalid regex");
        }
    };

    let result = re.find(&s).map(|m| m.as_str()).unwrap_or("").to_string();
    Box::into_raw(Box::new(CelValue::String(result)))
}

// ──────────────────────────────────────────────────────────────────────────────
// findAll with limit
// ──────────────────────────────────────────────────────────────────────────────

/// Returns up to `limit_ptr` substrings of `string_ptr` that match the regex
/// in `pattern_ptr` as an array of strings.
///
/// If `limit_ptr` is negative, all matches are returned (equivalent to
/// `findAll` without a limit).
///
/// Returns an error value if the string or pattern is not a String, the limit
/// is not an Int, or the regex is invalid.
///
/// # Safety
/// `string_ptr`, `pattern_ptr`, and `limit_ptr` must be valid, non-null
/// pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_regex_find_all_n(
    string_ptr: *mut CelValue,
    pattern_ptr: *mut CelValue,
    limit_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if string_ptr.is_null() || pattern_ptr.is_null() || limit_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_regex_find_all_n");
        return create_error_value("no such overload");
    }

    let string_val = unsafe { null_to_unbound(string_ptr) };
    let pattern_val = unsafe { null_to_unbound(pattern_ptr) };
    let limit_val = unsafe { null_to_unbound(limit_ptr) };

    let (s, pattern) = match (&string_val, &pattern_val) {
        (CelValue::String(s), CelValue::String(p)) => (s.clone(), p.clone()),
        _ => {
            error!(log, "expected String arguments";
                "function" => "cel_k8s_regex_find_all_n",
                "string" => format!("{:?}", string_val),
                "pattern" => format!("{:?}", pattern_val));
            return create_error_value("no such overload");
        }
    };

    let limit: i64 = match limit_val {
        CelValue::Int(n) => n,
        other => {
            error!(log, "expected Int limit";
                "function" => "cel_k8s_regex_find_all_n",
                "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let re = match Regex::new(&pattern) {
        Ok(r) => r,
        Err(e) => {
            error!(log, "invalid regex";
                "function" => "cel_k8s_regex_find_all_n",
                "pattern" => &pattern,
                "error" => format!("{}", e));
            return create_error_value("invalid regex");
        }
    };

    let matches: Vec<CelValue> = if limit < 0 {
        // Negative limit → return all matches
        re.find_iter(&s)
            .map(|m| CelValue::String(m.as_str().to_string()))
            .collect()
    } else {
        re.find_iter(&s)
            .take(limit as usize)
            .map(|m| CelValue::String(m.as_str().to_string()))
            .collect()
    };

    Box::into_raw(Box::new(CelValue::Array(matches)))
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{make_int, make_str, read_val};
    use super::*;
    use rstest::rstest;

    // ── find ────────────────────────────────────────────────────────────────

    #[rstest]
    #[case::basic_match("abc 123", r"[0-9]+", "123")]
    #[case::first_match_only("123 abc 456", r"[0-9]+", "123")]
    #[case::no_match("abc", r"[0-9]+", "")]
    #[case::empty_string("", r"[0-9]+", "")]
    #[case::anchored_start("hello world", r"^hello", "hello")]
    #[case::anchored_no_match("say hello", r"^hello", "")]
    #[case::full_string_match("hello", r"^hello$", "hello")]
    fn test_find(#[case] s: &str, #[case] pattern: &str, #[case] expected: &str) {
        let s_ptr = unsafe { make_str(s) };
        let p_ptr = unsafe { make_str(pattern) };
        let result = unsafe { read_val(cel_k8s_regex_find(s_ptr, p_ptr)) };
        assert_eq!(
            result,
            CelValue::String(expected.to_string()),
            "find({:?}, {:?})",
            s,
            pattern
        );
    }

    #[test]
    fn test_find_invalid_regex_returns_error() {
        let s_ptr = unsafe { make_str("hello") };
        let p_ptr = unsafe { make_str("[invalid") };
        let result = unsafe { read_val(cel_k8s_regex_find(s_ptr, p_ptr)) };
        assert!(
            matches!(result, CelValue::Error(_)),
            "expected error for invalid regex"
        );
    }

    // ── findAll (no limit — delegates to findAll_n with -1) ─────────────────

    #[rstest]
    #[case::multiple_matches(
        "123 abc 456",
        r"[0-9]+",
        vec!["123", "456"]
    )]
    #[case::no_matches(
        "abc def",
        r"[0-9]+",
        vec![]
    )]
    #[case::single_match(
        "only 1 number",
        r"[0-9]+",
        vec!["1"]
    )]
    #[case::empty_string(
        "",
        r"[0-9]+",
        vec![]
    )]
    fn test_find_all(#[case] s: &str, #[case] pattern: &str, #[case] expected: Vec<&str>) {
        let s_ptr = unsafe { make_str(s) };
        let p_ptr = unsafe { make_str(pattern) };
        let l_ptr = unsafe { make_int(-1) };
        let result = unsafe { read_val(cel_k8s_regex_find_all_n(s_ptr, p_ptr, l_ptr)) };
        let expected_val = CelValue::Array(
            expected
                .into_iter()
                .map(|e| CelValue::String(e.to_string()))
                .collect(),
        );
        assert_eq!(result, expected_val, "findAll({:?}, {:?})", s, pattern);
    }

    // ── findAll with limit ───────────────────────────────────────────────────

    #[rstest]
    #[case::limit_one("123 abc 456 def 789", r"[0-9]+", 1, vec!["123"])]
    #[case::limit_two("123 abc 456 def 789", r"[0-9]+", 2, vec!["123", "456"])]
    #[case::limit_exceeds_matches("123 abc 456", r"[0-9]+", 10, vec!["123", "456"])]
    #[case::limit_zero("123 abc 456", r"[0-9]+", 0, vec![])]
    #[case::limit_negative_all("123 abc 456", r"[0-9]+", -1, vec!["123", "456"])]
    fn test_find_all_n(
        #[case] s: &str,
        #[case] pattern: &str,
        #[case] limit: i64,
        #[case] expected: Vec<&str>,
    ) {
        let s_ptr = unsafe { make_str(s) };
        let p_ptr = unsafe { make_str(pattern) };
        let l_ptr = unsafe { make_int(limit) };
        let result = unsafe { read_val(cel_k8s_regex_find_all_n(s_ptr, p_ptr, l_ptr)) };
        let expected_val = CelValue::Array(
            expected
                .into_iter()
                .map(|e| CelValue::String(e.to_string()))
                .collect(),
        );
        assert_eq!(
            result, expected_val,
            "findAll({:?}, {:?}, {})",
            s, pattern, limit
        );
    }

    #[test]
    fn test_find_all_invalid_regex_returns_error() {
        let s_ptr = unsafe { make_str("hello") };
        let p_ptr = unsafe { make_str("[invalid") };
        let l_ptr = unsafe { make_int(-1) };
        let result = unsafe { read_val(cel_k8s_regex_find_all_n(s_ptr, p_ptr, l_ptr)) };
        assert!(
            matches!(result, CelValue::Error(_)),
            "expected error for invalid regex"
        );
    }
}
