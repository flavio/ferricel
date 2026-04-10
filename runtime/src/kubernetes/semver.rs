//! Kubernetes CEL semver library extensions.
//!
//! Implements the Kubernetes semver functions described in:
//!   <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-semver-library>
//!
//! Functions:
//!   - `isSemver(string)`                → bool
//!   - `isSemver(string, bool)`          → bool   (bool = normalize)
//!   - `semver(string)`                  → Semver (or error if invalid)
//!   - `semver(string, bool)`            → Semver (or error if invalid; bool = normalize)
//!   - `<Semver>.major()`                → int
//!   - `<Semver>.minor()`                → int
//!   - `<Semver>.patch()`                → int
//!   - `<Semver>.isLessThan(Semver)`     → bool
//!   - `<Semver>.isGreaterThan(Semver)`  → bool
//!   - `<Semver>.compareTo(Semver)`      → int   (-1, 0, or 1)
//!
//! Normalization logic (Go's `normalizeAndParse`):
//!   1. Strip a leading `v`
//!   2. Split on `.` into at most 3 parts
//!   3. For each numeric part (before `+`/`-`), strip leading zeros
//!   4. If fewer than 3 parts AND the last part contains `+` or `-`, return error
//!   5. Pad with `"0"` until 3 parts
//!   6. Rejoin and parse with `semver::Version::parse`

use crate::error::create_error_value;
use crate::types::CelValue;
use semver::Version;
use slog::error;

// ─────────────────────────────────────────────────────────────────────────────
// Parsing / normalization helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Strip leading zeros from a numeric string component.
/// If the component contains `.`, `+`, or `-` (i.e. it has a suffix), only
/// strip zeros from the numeric prefix.
///
/// Returns the cleaned string, or `None` if the numeric part is non-numeric.
fn strip_leading_zeros(part: &str) -> String {
    // Find where the numeric prefix ends (first non-digit character)
    let num_end = part
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(part.len());
    let numeric = &part[..num_end];
    let suffix = &part[num_end..];

    if numeric.is_empty() {
        // No numeric prefix at all — leave as-is
        return part.to_string();
    }

    // Strip leading zeros; keep at least one digit
    let stripped = numeric.trim_start_matches('0');
    let stripped = if stripped.is_empty() { "0" } else { stripped };
    format!("{}{}", stripped, suffix)
}

/// Parse a semver string, optionally normalizing it first.
///
/// Without normalization: only a strict semver string is accepted (no `v`
/// prefix, no leading zeros, exactly 3 numeric components).
///
/// With normalization (`normalize = true`):
///   1. Strip optional leading `v`
///   2. Split on `.` into at most 3 parts (use `splitN(s, ".", 3)` semantics)
///   3. Strip leading zeros from each numeric-only component
///   4. If fewer than 3 parts AND last part has `+` or `-`, return error
///   5. Pad with `"0"` to reach 3 parts
///   6. Parse with `semver::Version::parse`
fn normalize_and_parse(s: &str, normalize: bool) -> Result<Version, String> {
    if !normalize {
        // Strict parse — no leading `v`, no normalization
        Version::parse(s).map_err(|e| format!("semver: failed to parse: {}", e))
    } else {
        // Step 1: Strip optional leading `v`
        let s = s.strip_prefix('v').unwrap_or(s);

        // Step 2: Split on '.' into at most 3 parts
        // (Go's strings.SplitN(s, ".", 3) — last part may contain more dots but won't)
        let parts: Vec<&str> = s.splitn(3, '.').collect();

        // Step 3: Strip leading zeros from each part's numeric prefix
        let mut cleaned: Vec<String> = parts.iter().map(|p| strip_leading_zeros(p)).collect();

        // Step 4: If fewer than 3 parts and last part contains '+' or '-' → error
        if cleaned.len() < 3 {
            let last = cleaned.last().unwrap();
            if last.contains('+') || last.contains('-') {
                return Err(
                    "semver: short version cannot contain PreRelease/Build meta data".to_string(),
                );
            }
        }

        // Step 5: Pad with "0" to reach 3 parts
        while cleaned.len() < 3 {
            cleaned.push("0".to_string());
        }

        // Step 6: Parse
        let normalized = cleaned.join(".");
        Version::parse(&normalized)
            .map_err(|e| format!("semver: failed to parse '{}': {}", normalized, e))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// isSemver(string) / isSemver(string, bool)
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the string is a valid semver (strict, no normalization).
///
/// # Safety
/// `str_ptr` must be a valid, non-null pointer to a `CelValue::String`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_is_semver(str_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if str_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_is_semver");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*str_ptr };
    let s = match val {
        CelValue::String(s) => s.as_str(),
        other => {
            error!(log, "expected String"; "function" => "cel_k8s_is_semver", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let ok = normalize_and_parse(s, false).is_ok();
    Box::into_raw(Box::new(CelValue::Bool(ok)))
}

/// Returns `true` if the string is a valid semver, with optional normalization.
///
/// # Safety
/// `str_ptr` must be a valid, non-null pointer to a `CelValue::String`.
/// `normalize_ptr` must be a valid, non-null pointer to a `CelValue::Bool`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_is_semver_normalize(
    str_ptr: *mut CelValue,
    normalize_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if str_ptr.is_null() || normalize_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_is_semver_normalize");
        return create_error_value("no such overload");
    }

    let str_val = unsafe { &*str_ptr };
    let norm_val = unsafe { &*normalize_ptr };

    let s = match str_val {
        CelValue::String(s) => s.as_str(),
        other => {
            error!(log, "expected String as first argument"; "function" => "cel_k8s_is_semver_normalize", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let normalize = match norm_val {
        CelValue::Bool(b) => *b,
        other => {
            error!(log, "expected Bool as second argument"; "function" => "cel_k8s_is_semver_normalize", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let ok = normalize_and_parse(s, normalize).is_ok();
    Box::into_raw(Box::new(CelValue::Bool(ok)))
}

// ─────────────────────────────────────────────────────────────────────────────
// semver(string) / semver(string, bool)
// ─────────────────────────────────────────────────────────────────────────────

/// Converts a string to a Semver value (strict). Returns an error value if invalid.
///
/// # Safety
/// `str_ptr` must be a valid, non-null pointer to a `CelValue::String`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_semver_parse(str_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if str_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_semver_parse");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*str_ptr };
    let s = match val {
        CelValue::String(s) => s.as_str(),
        other => {
            error!(log, "expected String"; "function" => "cel_k8s_semver_parse", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    match normalize_and_parse(s, false) {
        Ok(v) => Box::into_raw(Box::new(CelValue::Semver(v))),
        Err(msg) => {
            error!(log, "invalid semver"; "function" => "cel_k8s_semver_parse", "error" => &msg);
            create_error_value(&msg)
        }
    }
}

/// Converts a string to a Semver value with optional normalization. Returns an error value if invalid.
///
/// # Safety
/// `str_ptr` must be a valid, non-null pointer to a `CelValue::String`.
/// `normalize_ptr` must be a valid, non-null pointer to a `CelValue::Bool`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_semver_parse_normalize(
    str_ptr: *mut CelValue,
    normalize_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if str_ptr.is_null() || normalize_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_semver_parse_normalize");
        return create_error_value("no such overload");
    }

    let str_val = unsafe { &*str_ptr };
    let norm_val = unsafe { &*normalize_ptr };

    let s = match str_val {
        CelValue::String(s) => s.as_str(),
        other => {
            error!(log, "expected String as first argument"; "function" => "cel_k8s_semver_parse_normalize", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let normalize = match norm_val {
        CelValue::Bool(b) => *b,
        other => {
            error!(log, "expected Bool as second argument"; "function" => "cel_k8s_semver_parse_normalize", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    match normalize_and_parse(s, normalize) {
        Ok(v) => Box::into_raw(Box::new(CelValue::Semver(v))),
        Err(msg) => {
            error!(log, "invalid semver"; "function" => "cel_k8s_semver_parse_normalize", "error" => &msg);
            create_error_value(&msg)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// major() / minor() / patch()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the major version component as an integer.
///
/// # Safety
/// `semver_ptr` must be a valid, non-null pointer to a `CelValue::Semver`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_semver_major(semver_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if semver_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_semver_major");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*semver_ptr };
    match val {
        CelValue::Semver(v) => Box::into_raw(Box::new(CelValue::Int(v.major as i64))),
        other => {
            error!(log, "expected Semver"; "function" => "cel_k8s_semver_major", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

/// Returns the minor version component as an integer.
///
/// # Safety
/// `semver_ptr` must be a valid, non-null pointer to a `CelValue::Semver`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_semver_minor(semver_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if semver_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_semver_minor");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*semver_ptr };
    match val {
        CelValue::Semver(v) => Box::into_raw(Box::new(CelValue::Int(v.minor as i64))),
        other => {
            error!(log, "expected Semver"; "function" => "cel_k8s_semver_minor", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

/// Returns the patch version component as an integer.
///
/// # Safety
/// `semver_ptr` must be a valid, non-null pointer to a `CelValue::Semver`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_semver_patch(semver_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if semver_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_semver_patch");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*semver_ptr };
    match val {
        CelValue::Semver(v) => Box::into_raw(Box::new(CelValue::Int(v.patch as i64))),
        other => {
            error!(log, "expected Semver"; "function" => "cel_k8s_semver_patch", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// isLessThan() / isGreaterThan() / compareTo()
//
// All three use `cmp_precedence()` which ignores build metadata (matching the
// Go blang/semver library behaviour).
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if `self` < `other` (precedence comparison, ignoring build metadata).
///
/// # Safety
/// Both pointers must be valid, non-null pointers to `CelValue::Semver`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_semver_is_less_than(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if lhs_ptr.is_null() || rhs_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_semver_is_less_than");
        return create_error_value("no such overload");
    }

    let lhs_val = unsafe { &*lhs_ptr };
    let rhs_val = unsafe { &*rhs_ptr };

    let lhs = match lhs_val {
        CelValue::Semver(v) => v,
        other => {
            error!(log, "expected Semver as first argument"; "function" => "cel_k8s_semver_is_less_than", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };
    let rhs = match rhs_val {
        CelValue::Semver(v) => v,
        other => {
            error!(log, "expected Semver as second argument"; "function" => "cel_k8s_semver_is_less_than", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let result = lhs.cmp_precedence(rhs) == std::cmp::Ordering::Less;
    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Returns `true` if `self` > `other` (precedence comparison, ignoring build metadata).
///
/// # Safety
/// Both pointers must be valid, non-null pointers to `CelValue::Semver`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_semver_is_greater_than(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if lhs_ptr.is_null() || rhs_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_semver_is_greater_than");
        return create_error_value("no such overload");
    }

    let lhs_val = unsafe { &*lhs_ptr };
    let rhs_val = unsafe { &*rhs_ptr };

    let lhs = match lhs_val {
        CelValue::Semver(v) => v,
        other => {
            error!(log, "expected Semver as first argument"; "function" => "cel_k8s_semver_is_greater_than", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };
    let rhs = match rhs_val {
        CelValue::Semver(v) => v,
        other => {
            error!(log, "expected Semver as second argument"; "function" => "cel_k8s_semver_is_greater_than", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let result = lhs.cmp_precedence(rhs) == std::cmp::Ordering::Greater;
    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Returns `-1`, `0`, or `1` comparing `self` to `other` by precedence (ignoring build metadata).
///
/// # Safety
/// Both pointers must be valid, non-null pointers to `CelValue::Semver`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_semver_compare_to(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if lhs_ptr.is_null() || rhs_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_semver_compare_to");
        return create_error_value("no such overload");
    }

    let lhs_val = unsafe { &*lhs_ptr };
    let rhs_val = unsafe { &*rhs_ptr };

    let lhs = match lhs_val {
        CelValue::Semver(v) => v,
        other => {
            error!(log, "expected Semver as first argument"; "function" => "cel_k8s_semver_compare_to", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };
    let rhs = match rhs_val {
        CelValue::Semver(v) => v,
        other => {
            error!(log, "expected Semver as second argument"; "function" => "cel_k8s_semver_compare_to", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let result: i64 = match lhs.cmp_precedence(rhs) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    };
    Box::into_raw(Box::new(CelValue::Int(result)))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{make_str, make_val, read_val};
    use super::*;
    use rstest::rstest;

    unsafe fn make_semver(s: &str) -> *mut CelValue {
        let str_ptr = unsafe { make_str(s) };
        unsafe { cel_k8s_semver_parse(str_ptr) }
    }

    unsafe fn make_semver_norm(s: &str) -> *mut CelValue {
        let str_ptr = unsafe { make_str(s) };
        let norm_ptr = unsafe { make_val(CelValue::Bool(true)) };
        unsafe { cel_k8s_semver_parse_normalize(str_ptr, norm_ptr) }
    }

    // ── normalize_and_parse ────────────────────────────────────────────────

    #[rstest]
    #[case("1.2.3", false, "1.2.3")]
    #[case("1.2.3-alpha", false, "1.2.3-alpha")]
    #[case("1.2.3+build", false, "1.2.3+build")]
    #[case("1.2.3-alpha+build", false, "1.2.3-alpha+build")]
    fn test_normalize_and_parse_strict_valid(
        #[case] input: &str,
        #[case] normalize: bool,
        #[case] expected: &str,
    ) {
        let v = normalize_and_parse(input, normalize).expect("should parse");
        assert_eq!(v.to_string(), expected);
    }

    #[rstest]
    #[case("v1.2.3", false)] // leading v not allowed without normalize
    #[case("1.2", false)] // missing patch
    #[case("not-semver", false)]
    fn test_normalize_and_parse_strict_invalid(#[case] input: &str, #[case] normalize: bool) {
        assert!(
            normalize_and_parse(input, normalize).is_err(),
            "expected Err for {:?}",
            input
        );
    }

    #[rstest]
    #[case("v1.2.3", true, "1.2.3")]
    #[case("1.2", true, "1.2.0")]
    #[case("1", true, "1.0.0")]
    #[case("01.01.01", true, "1.1.1")]
    #[case("v01.01", true, "1.1.0")]
    #[case("1.0.0-alpha", true, "1.0.0-alpha")]
    #[case("1.0.0+build", true, "1.0.0+build")]
    fn test_normalize_and_parse_normalized_valid(
        #[case] input: &str,
        #[case] normalize: bool,
        #[case] expected: &str,
    ) {
        let v = normalize_and_parse(input, normalize).expect("should parse");
        assert_eq!(v.to_string(), expected);
    }

    #[rstest]
    #[case("1-alpha", true)] // short version with pre-release
    #[case("1+build", true)] // short version with build metadata
    #[case("1.0-alpha", true)] // short version with pre-release
    fn test_normalize_and_parse_normalized_invalid(#[case] input: &str, #[case] normalize: bool) {
        assert!(
            normalize_and_parse(input, normalize).is_err(),
            "expected Err for {:?}",
            input
        );
    }

    // ── isSemver() / cel_k8s_is_semver ────────────────────────────────────

    #[rstest]
    #[case("1.2.3", true)]
    #[case("1.2.3-alpha", true)]
    #[case("1.2.3+build", true)]
    #[case("1.2.3-alpha+build", true)]
    #[case("v1.2.3", false)] // v prefix not allowed without normalize
    #[case("1.2", false)]
    #[case("not-a-semver", false)]
    fn test_is_semver(#[case] input: &str, #[case] expected: bool) {
        let str_ptr = unsafe { make_str(input) };
        let result = unsafe { read_val(cel_k8s_is_semver(str_ptr)) };
        assert_eq!(result, CelValue::Bool(expected), "isSemver({:?})", input);
        unsafe { drop(Box::from_raw(str_ptr)) };
    }

    // ── isSemver(string, bool) / cel_k8s_is_semver_normalize ──────────────

    #[rstest]
    #[case("1.2.3", false, true)]
    #[case("v1.2.3", true, true)]
    #[case("1.2", true, true)]
    #[case("1", true, true)]
    #[case("01.01.01", true, true)]
    #[case("1-alpha", true, false)] // short with pre-release
    #[case("v1.2.3", false, false)] // strict: no v prefix
    fn test_is_semver_normalize(
        #[case] input: &str,
        #[case] normalize: bool,
        #[case] expected: bool,
    ) {
        let str_ptr = unsafe { make_str(input) };
        let norm_ptr = unsafe { make_val(CelValue::Bool(normalize)) };
        let result = unsafe { read_val(cel_k8s_is_semver_normalize(str_ptr, norm_ptr)) };
        assert_eq!(
            result,
            CelValue::Bool(expected),
            "isSemver({:?}, {})",
            input,
            normalize
        );
        unsafe { drop(Box::from_raw(str_ptr)) };
    }

    // ── semver() / cel_k8s_semver_parse ───────────────────────────────────

    #[rstest]
    #[case("1.2.3")]
    #[case("1.2.3-alpha")]
    #[case("0.0.0")]
    fn test_semver_parse_valid(#[case] input: &str) {
        let str_ptr = unsafe { make_str(input) };
        let result = unsafe { read_val(cel_k8s_semver_parse(str_ptr)) };
        assert!(
            matches!(result, CelValue::Semver(_)),
            "expected Semver for {:?}, got {:?}",
            input,
            result
        );
        unsafe { drop(Box::from_raw(str_ptr)) };
    }

    #[rstest]
    #[case("v1.2.3")]
    #[case("1.2")]
    #[case("not-a-semver")]
    fn test_semver_parse_invalid(#[case] input: &str) {
        let str_ptr = unsafe { make_str(input) };
        let result = unsafe { read_val(cel_k8s_semver_parse(str_ptr)) };
        assert!(
            matches!(result, CelValue::Error(_)),
            "expected Error for {:?}, got {:?}",
            input,
            result
        );
        unsafe { drop(Box::from_raw(str_ptr)) };
    }

    // ── semver(string, bool) / cel_k8s_semver_parse_normalize ─────────────

    #[test]
    fn test_semver_parse_normalize_valid() {
        let str_ptr = unsafe { make_str("v01.02.03") };
        let norm_ptr = unsafe { make_val(CelValue::Bool(true)) };
        let result = unsafe { read_val(cel_k8s_semver_parse_normalize(str_ptr, norm_ptr)) };
        match result {
            CelValue::Semver(v) => {
                assert_eq!(v.major, 1);
                assert_eq!(v.minor, 2);
                assert_eq!(v.patch, 3);
            }
            other => panic!("expected Semver, got {:?}", other),
        }
        unsafe { drop(Box::from_raw(str_ptr)) };
    }

    // ── major() / minor() / patch() ───────────────────────────────────────

    #[test]
    fn test_semver_major() {
        let sv_ptr = unsafe { make_semver("3.5.1") };
        let result = unsafe { read_val(cel_k8s_semver_major(sv_ptr)) };
        assert_eq!(result, CelValue::Int(3));
    }

    #[test]
    fn test_semver_minor() {
        let sv_ptr = unsafe { make_semver("3.5.1") };
        let result = unsafe { read_val(cel_k8s_semver_minor(sv_ptr)) };
        assert_eq!(result, CelValue::Int(5));
    }

    #[test]
    fn test_semver_patch() {
        let sv_ptr = unsafe { make_semver("3.5.1") };
        let result = unsafe { read_val(cel_k8s_semver_patch(sv_ptr)) };
        assert_eq!(result, CelValue::Int(1));
    }

    // ── isLessThan() / isGreaterThan() / compareTo() ──────────────────────

    #[rstest]
    #[case("1.0.0", "2.0.0", true)]
    #[case("2.0.0", "1.0.0", false)]
    #[case("1.0.0", "1.0.0", false)]
    #[case("1.0.0-alpha", "1.0.0", true)] // pre-release < release
    fn test_is_less_than(#[case] lhs: &str, #[case] rhs: &str, #[case] expected: bool) {
        let lhs_ptr = unsafe { make_semver(lhs) };
        let rhs_ptr = unsafe { make_semver(rhs) };
        let result = unsafe { read_val(cel_k8s_semver_is_less_than(lhs_ptr, rhs_ptr)) };
        assert_eq!(
            result,
            CelValue::Bool(expected),
            "{}.isLessThan({})",
            lhs,
            rhs
        );
    }

    #[rstest]
    #[case("2.0.0", "1.0.0", true)]
    #[case("1.0.0", "2.0.0", false)]
    #[case("1.0.0", "1.0.0", false)]
    fn test_is_greater_than(#[case] lhs: &str, #[case] rhs: &str, #[case] expected: bool) {
        let lhs_ptr = unsafe { make_semver(lhs) };
        let rhs_ptr = unsafe { make_semver(rhs) };
        let result = unsafe { read_val(cel_k8s_semver_is_greater_than(lhs_ptr, rhs_ptr)) };
        assert_eq!(
            result,
            CelValue::Bool(expected),
            "{}.isGreaterThan({})",
            lhs,
            rhs
        );
    }

    #[rstest]
    #[case("1.0.0", "2.0.0", -1i64)]
    #[case("2.0.0", "1.0.0", 1i64)]
    #[case("1.0.0", "1.0.0", 0i64)]
    #[case("1.0.0+build.1", "1.0.0+build.2", 0i64)] // build metadata ignored in compareTo
    fn test_compare_to(#[case] lhs: &str, #[case] rhs: &str, #[case] expected: i64) {
        let lhs_ptr = unsafe { make_semver(lhs) };
        let rhs_ptr = unsafe { make_semver(rhs) };
        let result = unsafe { read_val(cel_k8s_semver_compare_to(lhs_ptr, rhs_ptr)) };
        assert_eq!(
            result,
            CelValue::Int(expected),
            "{}.compareTo({})",
            lhs,
            rhs
        );
    }

    // ── normalized semver major/minor/patch ───────────────────────────────

    #[test]
    fn test_normalized_semver_major_minor_patch() {
        let sv_ptr = unsafe { make_semver_norm("v01.01") };
        let major = unsafe { read_val(cel_k8s_semver_major(sv_ptr)) };
        assert_eq!(major, CelValue::Int(1), "major");

        let sv_ptr = unsafe { make_semver_norm("v01.01") };
        let minor = unsafe { read_val(cel_k8s_semver_minor(sv_ptr)) };
        assert_eq!(minor, CelValue::Int(1), "minor");

        let sv_ptr = unsafe { make_semver_norm("v01.01") };
        let patch = unsafe { read_val(cel_k8s_semver_patch(sv_ptr)) };
        assert_eq!(patch, CelValue::Int(0), "patch");
    }

    // ── wrong type returns error ──────────────────────────────────────────

    #[test]
    fn test_major_wrong_type_returns_error() {
        let val_ptr = unsafe { make_val(CelValue::String("1.2.3".to_string())) };
        let result = unsafe { read_val(cel_k8s_semver_major(val_ptr)) };
        assert!(matches!(result, CelValue::Error(_)));
        unsafe { drop(Box::from_raw(val_ptr)) };
    }

    #[test]
    fn test_is_less_than_wrong_type_returns_error() {
        let lhs_ptr = unsafe { make_val(CelValue::Int(1)) };
        let rhs_ptr = unsafe { make_semver("1.0.0") };
        let result = unsafe { read_val(cel_k8s_semver_is_less_than(lhs_ptr, rhs_ptr)) };
        assert!(matches!(result, CelValue::Error(_)));
        unsafe { drop(Box::from_raw(lhs_ptr)) };
    }
}
