//! Polymorphic `indexOf` / `lastIndexOf` for both strings and lists.
//!
//! These functions dispatch on the receiver type:
//! - `String` → substring search (codepoint index)
//! - `Array`  → element equality search (element index)

use super::strings::{find_index_of, find_last_index_of};
use crate::helpers::cel_equals;
use crate::types::CelValue;

/// Polymorphic `indexOf(arg)`:
/// - If receiver is a `String`, performs substring search (returns codepoint index or -1).
/// - If receiver is an `Array`, performs element search (returns element index or -1).
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_index_of_poly(
    receiver_ptr: *const CelValue,
    arg_ptr: *const CelValue,
) -> *mut CelValue {
    let receiver = unsafe { &*receiver_ptr };
    let arg = unsafe { &*arg_ptr };
    match receiver {
        CelValue::String(s) => {
            let sub = match arg {
                CelValue::String(s) => s.clone(),
                _ => {
                    return Box::into_raw(Box::new(CelValue::Error(
                        "indexOf: argument is not a string".to_string(),
                    )));
                }
            };
            let result = find_index_of(s, &sub, 0);
            Box::into_raw(Box::new(CelValue::Int(result)))
        }
        CelValue::Array(vec) => {
            for (i, elem) in vec.iter().enumerate() {
                if cel_equals(elem, arg) {
                    return Box::into_raw(Box::new(CelValue::Int(i as i64)));
                }
            }
            Box::into_raw(Box::new(CelValue::Int(-1)))
        }
        _ => Box::into_raw(Box::new(CelValue::Error(
            "indexOf: receiver must be a string or list".to_string(),
        ))),
    }
}

/// Polymorphic `lastIndexOf(arg)`:
/// - If receiver is a `String`, performs last substring search (returns codepoint index or -1).
/// - If receiver is an `Array`, performs last element search (returns element index or -1).
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_last_index_of_poly(
    receiver_ptr: *const CelValue,
    arg_ptr: *const CelValue,
) -> *mut CelValue {
    let receiver = unsafe { &*receiver_ptr };
    let arg = unsafe { &*arg_ptr };
    match receiver {
        CelValue::String(s) => {
            let sub = match arg {
                CelValue::String(s) => s.clone(),
                _ => {
                    return Box::into_raw(Box::new(CelValue::Error(
                        "lastIndexOf: argument is not a string".to_string(),
                    )));
                }
            };
            let cp_len = s.chars().count() as i64;
            let result = find_last_index_of(s, &sub, cp_len);
            Box::into_raw(Box::new(CelValue::Int(result)))
        }
        CelValue::Array(vec) => {
            for (i, elem) in vec.iter().enumerate().rev() {
                if cel_equals(elem, arg) {
                    return Box::into_raw(Box::new(CelValue::Int(i as i64)));
                }
            }
            Box::into_raw(Box::new(CelValue::Int(-1)))
        }
        _ => Box::into_raw(Box::new(CelValue::Error(
            "lastIndexOf: receiver must be a string or list".to_string(),
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deserialization::cel_free_value;
    use rstest::rstest;

    // ── cel_index_of_poly ─────────────────────────────────────────────────────

    #[rstest]
    #[case::string_found("tacocat", "ac", 1_i64)]
    #[case::string_not_found("tacocat", "none", -1_i64)]
    #[case::string_empty_needle("tacocat", "", 0_i64)]
    fn test_index_of_poly_string(#[case] s: &str, #[case] sub: &str, #[case] expected: i64) {
        let receiver = CelValue::String(s.to_string());
        let arg = CelValue::String(sub.to_string());
        unsafe {
            let result_ptr =
                cel_index_of_poly(&receiver as *const CelValue, &arg as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::Int(expected));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_index_of_poly_array_found() {
        let receiver = CelValue::Array(vec![
            CelValue::Int(10),
            CelValue::Int(20),
            CelValue::Int(30),
        ]);
        let arg = CelValue::Int(20);
        unsafe {
            let result_ptr =
                cel_index_of_poly(&receiver as *const CelValue, &arg as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::Int(1));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_index_of_poly_array_not_found() {
        let receiver = CelValue::Array(vec![CelValue::Int(1), CelValue::Int(2)]);
        let arg = CelValue::Int(99);
        unsafe {
            let result_ptr =
                cel_index_of_poly(&receiver as *const CelValue, &arg as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::Int(-1));
            cel_free_value(result_ptr);
        }
    }

    #[rstest]
    #[case::bool_receiver(CelValue::Bool(true))]
    #[case::int_receiver(CelValue::Int(42))]
    fn test_index_of_poly_wrong_receiver_returns_error(#[case] receiver: CelValue) {
        let arg = CelValue::Int(1);
        unsafe {
            let result_ptr =
                cel_index_of_poly(&receiver as *const CelValue, &arg as *const CelValue);
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }

    // ── cel_last_index_of_poly ────────────────────────────────────────────────

    #[rstest]
    #[case::string_found("tacocat", "at", 5_i64)]
    #[case::string_not_found("tacocat", "none", -1_i64)]
    #[case::string_empty_needle("tacocat", "", 7_i64)]
    fn test_last_index_of_poly_string(#[case] s: &str, #[case] sub: &str, #[case] expected: i64) {
        let receiver = CelValue::String(s.to_string());
        let arg = CelValue::String(sub.to_string());
        unsafe {
            let result_ptr =
                cel_last_index_of_poly(&receiver as *const CelValue, &arg as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::Int(expected));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_last_index_of_poly_array_last_occurrence() {
        // [10, 20, 10] — last 10 is at index 2
        let receiver = CelValue::Array(vec![
            CelValue::Int(10),
            CelValue::Int(20),
            CelValue::Int(10),
        ]);
        let arg = CelValue::Int(10);
        unsafe {
            let result_ptr =
                cel_last_index_of_poly(&receiver as *const CelValue, &arg as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::Int(2));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_last_index_of_poly_array_not_found() {
        let receiver = CelValue::Array(vec![CelValue::Int(1), CelValue::Int(2)]);
        let arg = CelValue::Int(99);
        unsafe {
            let result_ptr =
                cel_last_index_of_poly(&receiver as *const CelValue, &arg as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::Int(-1));
            cel_free_value(result_ptr);
        }
    }

    #[rstest]
    #[case::bool_receiver(CelValue::Bool(false))]
    #[case::int_receiver(CelValue::Int(0))]
    fn test_last_index_of_poly_wrong_receiver_returns_error(#[case] receiver: CelValue) {
        let arg = CelValue::Int(1);
        unsafe {
            let result_ptr =
                cel_last_index_of_poly(&receiver as *const CelValue, &arg as *const CelValue);
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }
}
