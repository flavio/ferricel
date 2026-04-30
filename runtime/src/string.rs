//! String operations for CEL runtime.
//!
//! This module provides string manipulation functions including:
//! - String creation from raw bytes
//! - String concatenation
//! - String length (size in Unicode codepoints)
//! - String comparison (startsWith, endsWith, contains)
//! - Regular expression matching (matches)

use crate::error::read_ptr;
use crate::types::CelValue;
use regex_lite::Regex;

/// Internal helper: Concatenate two strings.
///
/// # Arguments
/// - `a`: First string
/// - `b`: Second string
///
/// # Returns
/// A new String containing the concatenation of `a` and `b`
pub(crate) fn cel_string_concat(a: &str, b: &str) -> String {
    let mut result = String::with_capacity(a.len() + b.len());
    result.push_str(a);
    result.push_str(b);
    result
}

/// Creates a CelValue::String from a raw UTF-8 byte sequence.
///
/// # Arguments
/// - `data_ptr`: Pointer to UTF-8 bytes
/// - `len`: Length of the UTF-8 sequence in bytes
///
/// # Returns
/// Pointer to a heap-allocated CelValue::String
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - `data_ptr` points to valid UTF-8 bytes
/// - `len` is the correct length of the UTF-8 sequence
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_create_string(data_ptr: *const u8, len: usize) -> *mut CelValue {
    // Read the UTF-8 bytes from memory
    // SAFETY: Caller guarantees data_ptr is valid and len is correct
    let bytes = unsafe { core::slice::from_raw_parts(data_ptr, len) };

    // Convert to String - assuming valid UTF-8
    // In a production system, you might want to handle invalid UTF-8
    let string = String::from_utf8_lossy(bytes).into_owned();

    // Create CelValue::String and allocate on heap
    let value = Box::new(CelValue::String(string));
    Box::into_raw(value)
}

/// Returns the size of a string in Unicode codepoints.
///
/// # Arguments
/// - `string_ptr`: Pointer to a CelValue containing a string
///
/// # Returns
/// The number of Unicode codepoints in the string
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - `string_ptr` is a valid pointer to an initialized CelValue instance
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_string_size(string_ptr: *const CelValue) -> i64 {
    // SAFETY: Caller guarantees string_ptr is valid
    let value = unsafe { &*string_ptr };

    match value {
        CelValue::String(s) => s.chars().count() as i64,
        _ => 0, // Not a string, return 0
    }
}

/// Tests whether a string starts with a given prefix.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_starts_with(
    string_ptr: *mut CelValue,
    prefix_ptr: *mut CelValue,
) -> *mut CelValue {
    let s = unsafe { read_ptr(string_ptr) };
    let p = unsafe { read_ptr(prefix_ptr) };
    let result = match (s, p) {
        (CelValue::String(s), CelValue::String(p)) => CelValue::Bool(s.starts_with(&p)),
        _ => CelValue::Bool(false),
    };
    Box::into_raw(Box::new(result))
}

/// Tests whether a string ends with a given suffix.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_ends_with(
    string_ptr: *mut CelValue,
    suffix_ptr: *mut CelValue,
) -> *mut CelValue {
    let s = unsafe { read_ptr(string_ptr) };
    let p = unsafe { read_ptr(suffix_ptr) };
    let result = match (s, p) {
        (CelValue::String(s), CelValue::String(p)) => CelValue::Bool(s.ends_with(&p)),
        _ => CelValue::Bool(false),
    };
    Box::into_raw(Box::new(result))
}

/// Tests whether a string contains a given substring.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_contains(
    string_ptr: *mut CelValue,
    substring_ptr: *mut CelValue,
) -> *mut CelValue {
    let s = unsafe { read_ptr(string_ptr) };
    let sub = unsafe { read_ptr(substring_ptr) };
    let result = match (s, sub) {
        (CelValue::String(s), CelValue::String(sub)) => CelValue::Bool(s.contains(sub.as_str())),
        _ => CelValue::Bool(false),
    };
    Box::into_raw(Box::new(result))
}

/// Tests whether a string matches a regex pattern (RE2-compatible syntax).
///
/// Matches succeed if the pattern matches ANY substring of the input string.
/// Use anchors (^ and $) to force full-string matching.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers (must be String variants).
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_matches(
    string_ptr: *mut CelValue,
    pattern_ptr: *mut CelValue,
) -> *mut CelValue {
    let s = unsafe { read_ptr(string_ptr) };
    let p = unsafe { read_ptr(pattern_ptr) };
    let result = match (s, p) {
        (CelValue::String(s), CelValue::String(pattern)) => {
            let re = Regex::new(&pattern).expect("invalid regex pattern");
            CelValue::Bool(re.is_match(&s))
        }
        (CelValue::Null, _) | (_, CelValue::Null) => {
            panic!("Cannot match null values");
        }
        _ => {
            panic!("matches() expects String arguments");
        }
    };
    Box::into_raw(Box::new(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_create_string_logic() {
        let test_str = "hello world";
        let bytes = test_str.as_bytes();

        unsafe {
            let result_ptr = cel_create_string(bytes.as_ptr(), bytes.len());
            let result = &*result_ptr;

            match result {
                CelValue::String(s) => assert_eq!(s, "hello world"),
                _ => panic!("Expected String variant"),
            }
        }
    }

    #[test]
    fn test_create_string_with_unicode() {
        let test_str = "café ☕";
        let bytes = test_str.as_bytes();

        unsafe {
            let result_ptr = cel_create_string(bytes.as_ptr(), bytes.len());
            let result = &*result_ptr;

            match result {
                CelValue::String(s) => assert_eq!(s, "café ☕"),
                _ => panic!("Expected String variant"),
            }
        }
    }

    #[rstest]
    #[case::basic("hello", 5)]
    #[case::unicode("café", 4)]
    #[case::emoji("👋", 1)]
    fn test_string_size(#[case] input: &str, #[case] expected: i64) {
        let test_str = CelValue::String(input.to_string());

        unsafe {
            let size = cel_string_size(&test_str as *const CelValue);
            assert_eq!(size, expected);
        }
    }

    #[rstest]
    #[case::match_basic("hello world", "hello", true)]
    #[case::no_match("hello world", "world", false)]
    fn test_starts_with(#[case] string: &str, #[case] prefix: &str, #[case] expected: bool) {
        unsafe {
            let string_ptr = Box::into_raw(Box::new(CelValue::String(string.to_string())));
            let prefix_ptr = Box::into_raw(Box::new(CelValue::String(prefix.to_string())));
            let result_ptr = cel_string_starts_with(string_ptr, prefix_ptr);
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
        }
    }

    #[rstest]
    #[case::match_basic("hello world", "world", true)]
    #[case::no_match("hello world", "hello", false)]
    fn test_ends_with(#[case] string: &str, #[case] suffix: &str, #[case] expected: bool) {
        unsafe {
            let string_ptr = Box::into_raw(Box::new(CelValue::String(string.to_string())));
            let suffix_ptr = Box::into_raw(Box::new(CelValue::String(suffix.to_string())));
            let result_ptr = cel_string_ends_with(string_ptr, suffix_ptr);
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
        }
    }

    #[rstest]
    #[case::match_basic("hello world", "lo wo", true)]
    #[case::no_match("hello world", "xyz", false)]
    fn test_contains(#[case] string: &str, #[case] substring: &str, #[case] expected: bool) {
        unsafe {
            let string_ptr = Box::into_raw(Box::new(CelValue::String(string.to_string())));
            let substring_ptr = Box::into_raw(Box::new(CelValue::String(substring.to_string())));
            let result_ptr = cel_string_contains(string_ptr, substring_ptr);
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
        }
    }

    #[rstest]
    #[case::basic_match("foobar", "foo.*", true)]
    #[case::no_match("hello", "world", false)]
    #[case::substring("hello world", "wor", true)]
    #[case::anchored_start("foobar", "^foo", true)]
    #[case::anchored_end("foobar", "bar$", true)]
    #[case::full_anchored("foobar", "^foobar$", true)]
    #[case::character_class("abc123def", "[0-9]+", true)]
    #[case::quantifier("aaaa", "a{3,5}", true)]
    #[case::unicode("Hello 世界", "世界", true)]
    #[case::emoji("Hello 😀 World", "😀", true)]
    #[case::case_sensitive("Hello", "hello", false)]
    #[case::insensitive("Hello", "(?i)hello", true)]
    fn test_matches(#[case] string: &str, #[case] pattern: &str, #[case] expected: bool) {
        unsafe {
            let string_ptr = Box::into_raw(Box::new(CelValue::String(string.to_string())));
            let pattern_ptr = Box::into_raw(Box::new(CelValue::String(pattern.to_string())));
            let result_ptr = cel_string_matches(string_ptr, pattern_ptr);
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
        }
    }

    #[rstest]
    #[case::basic("hello", " world", "hello world")]
    #[case::empty_first("", "test", "test")]
    #[case::empty_second("test", "", "test")]
    #[case::unicode("Hello ", "世界", "Hello 世界")]
    #[case::emoji("Hello ", "👋🌍", "Hello 👋🌍")]
    fn test_string_concat(#[case] a: &str, #[case] b: &str, #[case] expected: &str) {
        let result = cel_string_concat(a, b);
        assert_eq!(result, expected);
    }

    // Note: Cannot test panic cases with #[should_panic] for extern "C" functions
    // as they cause process aborts. Panic behavior is tested in integration tests.
}
