//! CEL extended list library functions.
//!
//! Implements the `join` function from the CEL `strings` extension library,
//! which operates on lists of strings.

use crate::types::CelValue;

/// Joins a list of strings with no separator.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_list_join(list_ptr: *const CelValue) -> *mut CelValue {
    let list = match unsafe { &*list_ptr } {
        CelValue::Array(v) => v.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "join: receiver is not a list".to_string(),
            )));
        }
    };
    let mut result = String::new();
    for item in &list {
        match item {
            CelValue::String(s) => result.push_str(s),
            _ => {
                return Box::into_raw(Box::new(CelValue::Error(
                    "join: list contains non-string element".to_string(),
                )));
            }
        }
    }
    Box::into_raw(Box::new(CelValue::String(result)))
}

/// Joins a list of strings with a separator.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_list_join_sep(
    list_ptr: *const CelValue,
    sep_ptr: *const CelValue,
) -> *mut CelValue {
    let list = match unsafe { &*list_ptr } {
        CelValue::Array(v) => v.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "join: receiver is not a list".to_string(),
            )));
        }
    };
    let sep = match unsafe { &*sep_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "join: separator is not a string".to_string(),
            )));
        }
    };
    let mut result = String::new();
    for (i, item) in list.iter().enumerate() {
        match item {
            CelValue::String(s) => {
                if i > 0 {
                    result.push_str(&sep);
                }
                result.push_str(s);
            }
            _ => {
                return Box::into_raw(Box::new(CelValue::Error(
                    "join: list contains non-string element".to_string(),
                )));
            }
        }
    }
    Box::into_raw(Box::new(CelValue::String(result)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deserialization::cel_free_value;
    use rstest::rstest;

    // ── join (no separator) ───────────────────────────────────────────────────

    #[rstest]
    #[case::empty(vec![], "")]
    #[case::single(vec!["hello"], "hello")]
    #[case::multiple(vec!["a", "b", "c"], "abc")]
    fn test_list_join(#[case] items: Vec<&str>, #[case] expected: &str) {
        let list_val = CelValue::Array(
            items
                .iter()
                .map(|s| CelValue::String(s.to_string()))
                .collect(),
        );
        unsafe {
            let result_ptr = cel_list_join(&list_val as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::String(expected.to_string()));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_list_join_non_string_element_returns_error() {
        let list_val = CelValue::Array(vec![
            CelValue::String("hello".to_string()),
            CelValue::Int(42),
        ]);
        unsafe {
            let result_ptr = cel_list_join(&list_val as *const CelValue);
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }

    // ── join (with separator) ─────────────────────────────────────────────────

    #[rstest]
    #[case::empty(vec![], ",", "")]
    #[case::single(vec!["hello"], ",", "hello")]
    #[case::multiple(vec!["a", "b", "c"], ",", "a,b,c")]
    #[case::empty_sep(vec!["a", "b", "c"], "", "abc")]
    fn test_list_join_sep(#[case] items: Vec<&str>, #[case] sep: &str, #[case] expected: &str) {
        let list_val = CelValue::Array(
            items
                .iter()
                .map(|s| CelValue::String(s.to_string()))
                .collect(),
        );
        let sep_val = CelValue::String(sep.to_string());
        unsafe {
            let result_ptr =
                cel_list_join_sep(&list_val as *const CelValue, &sep_val as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::String(expected.to_string()));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_list_join_sep_non_string_element_returns_error() {
        let list_val = CelValue::Array(vec![
            CelValue::String("a".to_string()),
            CelValue::Bool(true),
        ]);
        let sep_val = CelValue::String("-".to_string());
        unsafe {
            let result_ptr =
                cel_list_join_sep(&list_val as *const CelValue, &sep_val as *const CelValue);
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }
}
