//! CEL regex extension library functions.
//!
//! Implements the cel-go `ext.Regex()` functions:
//!   - `regex.replace(target, pattern, replacement) -> string`
//!   - `regex.replace(target, pattern, replacement, count) -> string`
//!   - `regex.extract(target, pattern) -> optional<string>`
//!   - `regex.extractAll(target, pattern) -> list<string>`
//!
//! Reference: <https://pkg.go.dev/github.com/google/cel-go/ext#Regex>

use regex_lite::Regex;
use slog::error;

use crate::{error::create_error_value, types::CelValue};

// ─────────────────────────────────────────────────────────────────────────────
// Replacement string conversion: CEL → regex_lite
//
// CEL uses `\1`–`\9` for capture group references and `\\` for a literal
// backslash.  regex_lite uses `$1`–`$9` and `$$` respectively.  Any dollar
// signs in the original string must be escaped to `$$` so they are treated as
// literals.
// ─────────────────────────────────────────────────────────────────────────────

/// Convert a CEL replacement string to a `regex_lite` replacement string.
///
/// Mapping:
/// - `\\`  → `\`   (literal backslash in output)
/// - `\N`  (N = 1–9) → `$N` (capture group reference)
/// - `\`   followed by anything else → error
/// - `$`   → `$$` (escape so regex_lite treats it as a literal)
/// - everything else → copied verbatim
fn cel_replacement_to_regex_lite(cel_repl: &str) -> Result<String, String> {
    let mut out = String::with_capacity(cel_repl.len() * 2);
    let mut chars = cel_repl.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '$' => {
                // Escape bare dollar signs so regex_lite doesn't interpret them
                out.push_str("$$");
            }
            '\\' => match chars.next() {
                Some('\\') => out.push('\\'),
                Some(d @ '1'..='9') => {
                    out.push('$');
                    out.push(d);
                }
                Some(other) => {
                    return Err(format!(
                        "invalid replacement string: \\{} is not a valid escape",
                        other
                    ));
                }
                None => {
                    return Err("invalid replacement string: trailing backslash".to_string());
                }
            },
            other => out.push(other),
        }
    }
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// regex.replace(target, pattern, replacement) -> string
// ─────────────────────────────────────────────────────────────────────────────

/// `regex.replace(target, pattern, replacement) -> string`
///
/// Replaces all non-overlapping matches of `pattern` in `target` with
/// `replacement`.  The replacement string may reference capture groups via
/// `\1`–`\9`.
///
/// Returns an error value if any argument is not a String, the regex is
/// invalid, or the replacement string contains an invalid escape.
///
/// # Safety
/// All pointer arguments must be valid non-null pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_regex_replace(
    target_ptr: *mut CelValue,
    pattern_ptr: *mut CelValue,
    replacement_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if target_ptr.is_null() || pattern_ptr.is_null() || replacement_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_regex_replace");
        return create_error_value("no such overload");
    }

    let target_val = unsafe { &*target_ptr };
    let pattern_val = unsafe { &*pattern_ptr };
    let replacement_val = unsafe { &*replacement_ptr };

    let (target, pattern, replacement) = match (target_val, pattern_val, replacement_val) {
        (CelValue::String(t), CelValue::String(p), CelValue::String(r)) => {
            (t.as_str(), p.as_str(), r.as_str())
        }
        _ => {
            error!(log, "expected String arguments"; "function" => "cel_regex_replace");
            return create_error_value("no such overload");
        }
    };

    let re = match Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => {
            error!(log, "invalid regex";
                "function" => "cel_regex_replace",
                "pattern" => pattern,
                "error" => format!("{}", e));
            return create_error_value(&format!("invalid regex: {}", e));
        }
    };

    let repl = match cel_replacement_to_regex_lite(replacement) {
        Ok(r) => r,
        Err(e) => {
            error!(log, "invalid replacement string";
                "function" => "cel_regex_replace",
                "error" => &e);
            return create_error_value(&e);
        }
    };

    let result = re.replace_all(target, repl.as_str()).into_owned();
    Box::into_raw(Box::new(CelValue::String(result)))
}

// ─────────────────────────────────────────────────────────────────────────────
// regex.replace(target, pattern, replacement, count) -> string
// ─────────────────────────────────────────────────────────────────────────────

/// `regex.replace(target, pattern, replacement, count) -> string`
///
/// Replaces up to `count` non-overlapping matches of `pattern` in `target`.
///
/// - `count == 0`: returns `target` unchanged.
/// - `count < 0`: replaces all matches (same as `regex.replace` without count).
/// - `count > 0`: replaces at most `count` matches.
///
/// # Safety
/// All pointer arguments must be valid non-null pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_regex_replace_n(
    target_ptr: *mut CelValue,
    pattern_ptr: *mut CelValue,
    replacement_ptr: *mut CelValue,
    count_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if target_ptr.is_null()
        || pattern_ptr.is_null()
        || replacement_ptr.is_null()
        || count_ptr.is_null()
    {
        error!(log, "null pointer"; "function" => "cel_regex_replace_n");
        return create_error_value("no such overload");
    }

    let target_val = unsafe { &*target_ptr };
    let pattern_val = unsafe { &*pattern_ptr };
    let replacement_val = unsafe { &*replacement_ptr };
    let count_val = unsafe { &*count_ptr };

    let (target, pattern, replacement) = match (target_val, pattern_val, replacement_val) {
        (CelValue::String(t), CelValue::String(p), CelValue::String(r)) => {
            (t.as_str(), p.as_str(), r.as_str())
        }
        _ => {
            error!(log, "expected String arguments"; "function" => "cel_regex_replace_n");
            return create_error_value("no such overload");
        }
    };

    let count: i64 = match count_val {
        CelValue::Int(n) => *n,
        other => {
            error!(log, "expected Int count";
                "function" => "cel_regex_replace_n",
                "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    // count == 0: return target unchanged
    if count == 0 {
        return Box::into_raw(Box::new(CelValue::String(target.to_string())));
    }

    let re = match Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => {
            error!(log, "invalid regex";
                "function" => "cel_regex_replace_n",
                "pattern" => pattern,
                "error" => format!("{}", e));
            return create_error_value(&format!("invalid regex: {}", e));
        }
    };

    let repl = match cel_replacement_to_regex_lite(replacement) {
        Ok(r) => r,
        Err(e) => {
            error!(log, "invalid replacement string";
                "function" => "cel_regex_replace_n",
                "error" => &e);
            return create_error_value(&e);
        }
    };

    let result = if count < 0 {
        re.replace_all(target, repl.as_str()).into_owned()
    } else {
        // replace_n replaces exactly the first `count` matches
        re.replacen(target, count as usize, repl.as_str())
            .into_owned()
    };

    Box::into_raw(Box::new(CelValue::String(result)))
}

// ─────────────────────────────────────────────────────────────────────────────
// regex.extract(target, pattern) -> optional<string>
// ─────────────────────────────────────────────────────────────────────────────

/// `regex.extract(target, pattern) -> optional<string>`
///
/// Returns the first match of `pattern` in `target` as `optional.of(match)`,
/// or `optional.none()` if there is no match.
///
/// If `pattern` contains exactly one capture group, the content of that group
/// is returned rather than the full match.
///
/// Returns an error value if the pattern has two or more capture groups, if
/// either argument is not a String, or if the regex is invalid.
///
/// # Safety
/// Both pointer arguments must be valid non-null pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_regex_extract(
    target_ptr: *mut CelValue,
    pattern_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if target_ptr.is_null() || pattern_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_regex_extract");
        return create_error_value("no such overload");
    }

    let target_val = unsafe { &*target_ptr };
    let pattern_val = unsafe { &*pattern_ptr };

    let (target, pattern) = match (target_val, pattern_val) {
        (CelValue::String(t), CelValue::String(p)) => (t.as_str(), p.as_str()),
        _ => {
            error!(log, "expected String arguments"; "function" => "cel_regex_extract");
            return create_error_value("no such overload");
        }
    };

    let re = match Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => {
            error!(log, "invalid regex";
                "function" => "cel_regex_extract",
                "pattern" => pattern,
                "error" => format!("{}", e));
            return create_error_value(&format!("invalid regex: {}", e));
        }
    };

    // Count explicit capture groups (excludes group 0, the full match)
    let num_captures = re.captures_len().saturating_sub(1);

    if num_captures > 1 {
        return create_error_value("regex.extract: pattern must have at most one capture group");
    }

    match re.captures(target) {
        None => Box::into_raw(Box::new(CelValue::Optional(None))),
        Some(caps) => {
            let result = if num_captures == 1 {
                // Return the captured group (group 1)
                caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string()
            } else {
                // No capture groups — return the full match
                caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string()
            };
            Box::into_raw(Box::new(CelValue::Optional(Some(Box::new(
                CelValue::String(result),
            )))))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// regex.extractAll(target, pattern) -> list<string>
// ─────────────────────────────────────────────────────────────────────────────

/// `regex.extractAll(target, pattern) -> list<string>`
///
/// Returns all non-overlapping matches of `pattern` in `target` as a list.
///
/// If `pattern` contains exactly one capture group, the content of each
/// captured group is returned rather than the full match.  Returns an empty
/// list when there are no matches.
///
/// Returns an error value if the pattern has two or more capture groups, if
/// either argument is not a String, or if the regex is invalid.
///
/// # Safety
/// Both pointer arguments must be valid non-null pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_regex_extract_all(
    target_ptr: *mut CelValue,
    pattern_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if target_ptr.is_null() || pattern_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_regex_extract_all");
        return create_error_value("no such overload");
    }

    let target_val = unsafe { &*target_ptr };
    let pattern_val = unsafe { &*pattern_ptr };

    let (target, pattern) = match (target_val, pattern_val) {
        (CelValue::String(t), CelValue::String(p)) => (t.as_str(), p.as_str()),
        _ => {
            error!(log, "expected String arguments"; "function" => "cel_regex_extract_all");
            return create_error_value("no such overload");
        }
    };

    let re = match Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => {
            error!(log, "invalid regex";
                "function" => "cel_regex_extract_all",
                "pattern" => pattern,
                "error" => format!("{}", e));
            return create_error_value(&format!("invalid regex: {}", e));
        }
    };

    let num_captures = re.captures_len().saturating_sub(1);

    if num_captures > 1 {
        return create_error_value("regex.extractAll: pattern must have at most one capture group");
    }

    let results: Vec<CelValue> = re
        .captures_iter(target)
        .map(|caps| {
            let s = if num_captures == 1 {
                caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string()
            } else {
                caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string()
            };
            CelValue::String(s)
        })
        .collect();

    Box::into_raw(Box::new(CelValue::Array(results)))
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_helpers::{make_int, make_str, none, read_val, some_str, strs};
    use crate::types::CelValue;

    // ── cel_replacement_to_regex_lite ────────────────────────────────────────

    #[rstest]
    #[case::plain("hello", "hello")]
    #[case::capture_group_1(r"\1", "$1")]
    #[case::capture_group_9(r"\9", "$9")]
    #[case::literal_backslash(r"\\", r"\")]
    #[case::dollar_escaped("a$b", "a$$b")]
    // escaping edge cases
    #[case::empty("", "")]
    #[case::bare_dollar("$", "$$")]
    #[case::two_dollars("$$", "$$$$")]
    // CEL `$\1`  →  regex_lite `$$$1`  (literal `$` then capture group 1)
    #[case::dollar_then_group("$\\1", "$$$1")]
    // CEL `\\\1`  →  regex_lite `\$1`  (literal `\` then capture group 1)
    #[case::backslash_then_group("\\\\\\1", "\\$1")]
    fn test_repl_conversion_ok(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(cel_replacement_to_regex_lite(input).unwrap(), expected);
    }

    #[rstest]
    #[case::invalid_escape(r"\x")]
    #[case::trailing_backslash(r"\")]
    fn test_repl_conversion_err(#[case] input: &str) {
        assert!(cel_replacement_to_regex_lite(input).is_err());
    }

    // ── regex.replace (all matches) ──────────────────────────────────────────

    #[rstest]
    #[case::basic("hello world hello", "hello", "bye", "bye world bye")]
    #[case::no_match("hello", r"\d+", "X", "hello")]
    #[case::capture_group("2024-01-15", r"(\d{4})-(\d{2})-(\d{2})", r"\3/\2/\1", "15/01/2024")]
    // $ and \ escaping must survive through to the actual replace output
    // CEL `$99` → literal "$99" in output (not a capture group reference)
    #[case::literal_dollar("price 100", r"\d+", "$99", "price $99")]
    // CEL `$$` → literal "$$" in output
    #[case::two_literal_dollars("x", "x", "$$", "$$")]
    // CEL `\\` → literal "\" in output
    #[case::literal_backslash_in_output("abc", "b", r"\\", r"a\c")]
    // CEL `$\1` → literal "$" then capture group 1 content
    #[case::literal_dollar_then_capture("abc", "(b)", "$\\1", "a$bc")]
    // CEL `\\\1` → literal "\" then capture group 1 content
    #[case::literal_backslash_then_capture("abc", "(b)", "\\\\\\1", r"a\bc")]
    // Target contains "$": pattern matches it and replacement has no groups
    #[case::dollar_in_target("$100", r"\$(\d+)", r"\1", "100")]
    // Empty replacement removes the match
    #[case::empty_replacement("abc", "b", "", "ac")]
    fn test_replace(
        #[case] target: &str,
        #[case] pattern: &str,
        #[case] replacement: &str,
        #[case] expected: &str,
    ) {
        let t = make_str(target);
        let p = make_str(pattern);
        let r = make_str(replacement);
        let result = read_val(unsafe { cel_regex_replace(t, p, r) });
        assert_eq!(result, CelValue::String(expected.to_string()));
        unsafe {
            drop(Box::from_raw(t));
            drop(Box::from_raw(p));
            drop(Box::from_raw(r));
        }
    }

    #[test]
    fn test_replace_invalid_regex_returns_error() {
        let t = make_str("hello");
        let p = make_str("[invalid");
        let r = make_str("X");
        let result = read_val(unsafe { cel_regex_replace(t, p, r) });
        assert!(matches!(result, CelValue::Error(_)));
        unsafe {
            drop(Box::from_raw(t));
            drop(Box::from_raw(p));
            drop(Box::from_raw(r));
        }
    }

    // ── regex.replace (with count) ───────────────────────────────────────────

    #[rstest]
    #[case::count_zero_noop("aaa", "a", "b", 0, "aaa")]
    #[case::count_one("aaa", "a", "b", 1, "baa")]
    #[case::count_two("aaa", "a", "b", 2, "bba")]
    #[case::count_negative_all("aaa", "a", "b", -1, "bbb")]
    // Escaping must work through the counted-replace path too
    // CEL `$` → literal "$" even when count limits replacements
    #[case::literal_dollar_count_1("a1 a2", r"\d", "$", 1, "a$ a2")]
    // CEL `\\` → literal "\" for all matches via negative count
    #[case::literal_backslash_count_neg("abc", "[abc]", r"\\", -1, r"\\\")]
    fn test_replace_n(
        #[case] target: &str,
        #[case] pattern: &str,
        #[case] replacement: &str,
        #[case] count: i64,
        #[case] expected: &str,
    ) {
        let t = make_str(target);
        let p = make_str(pattern);
        let r = make_str(replacement);
        let c = make_int(count);
        let result = read_val(unsafe { cel_regex_replace_n(t, p, r, c) });
        assert_eq!(result, CelValue::String(expected.to_string()));
        unsafe {
            drop(Box::from_raw(t));
            drop(Box::from_raw(p));
            drop(Box::from_raw(r));
            drop(Box::from_raw(c));
        }
    }

    // ── regex.extract ────────────────────────────────────────────────────────

    #[rstest]
    #[case::no_capture_group("hello 123 world", r"\d+", some_str("123"))]
    #[case::with_capture_group("hello 123 world", r"hello (\d+)", some_str("123"))]
    #[case::no_match("hello world", r"\d+", none())]
    fn test_extract(#[case] target: &str, #[case] pattern: &str, #[case] expected: CelValue) {
        let t = make_str(target);
        let p = make_str(pattern);
        let result = read_val(unsafe { cel_regex_extract(t, p) });
        assert_eq!(result, expected);
        unsafe {
            drop(Box::from_raw(t));
            drop(Box::from_raw(p));
        }
    }

    #[test]
    fn test_extract_two_capture_groups_returns_error() {
        let t = make_str("hello 123");
        let p = make_str(r"(hello) (\d+)");
        let result = read_val(unsafe { cel_regex_extract(t, p) });
        assert!(matches!(result, CelValue::Error(_)));
        unsafe {
            drop(Box::from_raw(t));
            drop(Box::from_raw(p));
        }
    }

    // ── regex.extractAll ─────────────────────────────────────────────────────

    #[rstest]
    #[case::no_capture_group("abc 123 def 456", r"\d+", strs(&["123", "456"]))]
    #[case::with_capture_group("key=val1 key=val2", r"key=(\w+)", strs(&["val1", "val2"]))]
    #[case::no_matches("hello world", r"\d+", strs(&[]))]
    fn test_extract_all(#[case] target: &str, #[case] pattern: &str, #[case] expected: CelValue) {
        let t = make_str(target);
        let p = make_str(pattern);
        let result = read_val(unsafe { cel_regex_extract_all(t, p) });
        assert_eq!(result, expected);
        unsafe {
            drop(Box::from_raw(t));
            drop(Box::from_raw(p));
        }
    }

    #[test]
    fn test_extract_all_two_capture_groups_returns_error() {
        let t = make_str("hello 123");
        let p = make_str(r"(hello) (\d+)");
        let result = read_val(unsafe { cel_regex_extract_all(t, p) });
        assert!(matches!(result, CelValue::Error(_)));
        unsafe {
            drop(Box::from_raw(t));
            drop(Box::from_raw(p));
        }
    }
}
