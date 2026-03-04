//! String operations for CEL runtime.
//!
//! This module provides string manipulation functions including:
//! - String creation from raw bytes
//! - String concatenation
//! - String length (size in Unicode codepoints)
//! - String comparison (startsWith, endsWith, contains)
//! - Regular expression matching (matches)

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
/// - The returned pointer must be freed using the appropriate cleanup function
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_size(string_ptr: *const CelValue) -> i64 {
    // SAFETY: Caller guarantees string_ptr is valid
    let value = unsafe { &*string_ptr };

    match value {
        CelValue::String(s) => s.chars().count() as i64,
        _ => 0, // Not a string, return 0
    }
}

/// Tests whether a string starts with a given prefix.
///
/// # Arguments
/// - `string_ptr`: Pointer to the string to test
/// - `prefix_ptr`: Pointer to the prefix to check
///
/// # Returns
/// Pointer to CelValue::Bool(true) if string starts with prefix, false otherwise
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_starts_with(
    string_ptr: *const CelValue,
    prefix_ptr: *const CelValue,
) -> *mut CelValue {
    // SAFETY: Caller guarantees both pointers are valid
    let string_val = unsafe { &*string_ptr };
    let prefix_val = unsafe { &*prefix_ptr };

    let result = match (string_val, prefix_val) {
        (CelValue::String(s), CelValue::String(prefix)) => s.starts_with(prefix),
        _ => false,
    };

    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Tests whether a string ends with a given suffix.
///
/// # Arguments
/// - `string_ptr`: Pointer to the string to test
/// - `suffix_ptr`: Pointer to the suffix to check
///
/// # Returns
/// Pointer to CelValue::Bool(true) if string ends with suffix, false otherwise
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_ends_with(
    string_ptr: *const CelValue,
    suffix_ptr: *const CelValue,
) -> *mut CelValue {
    // SAFETY: Caller guarantees both pointers are valid
    let string_val = unsafe { &*string_ptr };
    let suffix_val = unsafe { &*suffix_ptr };

    let result = match (string_val, suffix_val) {
        (CelValue::String(s), CelValue::String(suffix)) => s.ends_with(suffix),
        _ => false,
    };

    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Tests whether a string contains a given substring.
///
/// # Arguments
/// - `string_ptr`: Pointer to the string to test
/// - `substring_ptr`: Pointer to the substring to find
///
/// # Returns
/// Pointer to CelValue::Bool(true) if string contains substring, false otherwise
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_contains(
    string_ptr: *const CelValue,
    substring_ptr: *const CelValue,
) -> *mut CelValue {
    // SAFETY: Caller guarantees both pointers are valid
    let string_val = unsafe { &*string_ptr };
    let substring_val = unsafe { &*substring_ptr };

    let result = match (string_val, substring_val) {
        (CelValue::String(s), CelValue::String(substring)) => s.contains(substring.as_str()),
        _ => false,
    };

    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Tests whether a string matches a regex pattern (RE2-compatible syntax).
///
/// Matches succeed if the pattern matches ANY substring of the input string.
/// Use anchors (^ and $) to force full-string matching.
///
/// # Arguments
/// - `string_ptr`: Pointer to a CelValue containing the string to test
/// - `pattern_ptr`: Pointer to a CelValue containing the regex pattern
///
/// # Returns
/// Pointer to a heap-allocated CelValue::Bool
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue instances (must be String variants)
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_matches(
    string_ptr: *const CelValue,
    pattern_ptr: *const CelValue,
) -> *mut CelValue {
    // SAFETY: Caller guarantees both pointers are valid
    let string_val = unsafe { &*string_ptr };
    let pattern_val = unsafe { &*pattern_ptr };

    match (string_val, pattern_val) {
        (CelValue::String(s), CelValue::String(pattern)) => {
            // Compile the regex pattern - panic on invalid patterns (CEL error)
            let re = Regex::new(pattern).expect("invalid regex pattern");
            let result = re.is_match(s);
            Box::into_raw(Box::new(CelValue::Bool(result)))
        }
        (CelValue::Null, _) | (_, CelValue::Null) => {
            panic!("Cannot match null values");
        }
        _ => {
            panic!("matches() expects String arguments");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deserialization::cel_free_value;
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

            // Clean up
            cel_free_value(result_ptr);
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

            cel_free_value(result_ptr);
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
        let string_val = CelValue::String(string.to_string());
        let prefix_val = CelValue::String(prefix.to_string());

        unsafe {
            let result_ptr = cel_string_starts_with(
                &string_val as *const CelValue,
                &prefix_val as *const CelValue,
            );
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
            cel_free_value(result_ptr);
        }
    }

    #[rstest]
    #[case::match_basic("hello world", "world", true)]
    #[case::no_match("hello world", "hello", false)]
    fn test_ends_with(#[case] string: &str, #[case] suffix: &str, #[case] expected: bool) {
        let string_val = CelValue::String(string.to_string());
        let suffix_val = CelValue::String(suffix.to_string());

        unsafe {
            let result_ptr = cel_string_ends_with(
                &string_val as *const CelValue,
                &suffix_val as *const CelValue,
            );
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
            cel_free_value(result_ptr);
        }
    }

    #[rstest]
    #[case::match_basic("hello world", "lo wo", true)]
    #[case::no_match("hello world", "xyz", false)]
    fn test_contains(#[case] string: &str, #[case] substring: &str, #[case] expected: bool) {
        let string_val = CelValue::String(string.to_string());
        let substring_val = CelValue::String(substring.to_string());

        unsafe {
            let result_ptr = cel_string_contains(
                &string_val as *const CelValue,
                &substring_val as *const CelValue,
            );
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
            cel_free_value(result_ptr);
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
        let string_val = CelValue::String(string.to_string());
        let pattern_val = CelValue::String(pattern.to_string());

        unsafe {
            let result_ptr = cel_string_matches(
                &string_val as *const CelValue,
                &pattern_val as *const CelValue,
            );
            let result = &*result_ptr;

            assert_eq!(result, &CelValue::Bool(expected));
            cel_free_value(result_ptr);
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
