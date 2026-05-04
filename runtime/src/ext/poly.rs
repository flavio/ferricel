//! Polymorphic functions overloaded by receiver type:
//! - `indexOf` / `lastIndexOf`: `String` → substring search; `Array` → element search
//! - `reverse`: `String` → character reversal; `Array` → element reversal

use super::{
    lists::list_reverse_impl,
    strings::{find_index_of, find_last_index_of, string_reverse_impl},
};
use crate::{error::read_ptr, helpers::cel_equals, types::CelValue};

/// Polymorphic `reverse`:
/// - If receiver is a `String`, reverses the Unicode characters.
/// - If receiver is an `Array`, reverses the element order.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_reverse_poly(receiver_ptr: *mut CelValue) -> *mut CelValue {
    let receiver = unsafe { read_ptr(receiver_ptr) };
    match receiver {
        CelValue::String(s) => Box::into_raw(Box::new(string_reverse_impl(s))),
        CelValue::Array(v) => Box::into_raw(Box::new(list_reverse_impl(v))),
        _ => Box::into_raw(Box::new(CelValue::Error(
            "reverse: receiver must be a string or list".to_string(),
        ))),
    }
}

/// Polymorphic `indexOf(arg)`:
/// - If receiver is a `String`, performs substring search (returns codepoint index or -1).
/// - If receiver is an `Array`, performs element search (returns element index or -1).
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_index_of_poly(
    receiver_ptr: *mut CelValue,
    arg_ptr: *mut CelValue,
) -> *mut CelValue {
    let receiver = unsafe { read_ptr(receiver_ptr) };
    let arg = unsafe { read_ptr(arg_ptr) };
    match receiver {
        CelValue::String(s) => {
            let sub = match arg {
                CelValue::String(s) => s,
                _ => {
                    return Box::into_raw(Box::new(CelValue::Error(
                        "indexOf: argument is not a string".to_string(),
                    )));
                }
            };
            let result = find_index_of(&s, &sub, 0);
            Box::into_raw(Box::new(CelValue::Int(result)))
        }
        CelValue::Array(vec) => {
            for (i, elem) in vec.iter().enumerate() {
                if cel_equals(elem, &arg) {
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
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_last_index_of_poly(
    receiver_ptr: *mut CelValue,
    arg_ptr: *mut CelValue,
) -> *mut CelValue {
    let receiver = unsafe { read_ptr(receiver_ptr) };
    let arg = unsafe { read_ptr(arg_ptr) };
    match receiver {
        CelValue::String(s) => {
            let sub = match arg {
                CelValue::String(s) => s,
                _ => {
                    return Box::into_raw(Box::new(CelValue::Error(
                        "lastIndexOf: argument is not a string".to_string(),
                    )));
                }
            };
            let cp_len = s.chars().count() as i64;
            let result = find_last_index_of(&s, &sub, cp_len);
            Box::into_raw(Box::new(CelValue::Int(result)))
        }
        CelValue::Array(vec) => {
            for (i, elem) in vec.iter().enumerate().rev() {
                if cel_equals(elem, &arg) {
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
    use rstest::rstest;

    use super::*;

    // ── cel_index_of_poly ─────────────────────────────────────────────────────

    #[rstest]
    #[case::string_found("tacocat", "ac", 1_i64)]
    #[case::string_not_found("tacocat", "none", -1_i64)]
    #[case::string_empty_needle("tacocat", "", 0_i64)]
    fn test_index_of_poly_string(#[case] s: &str, #[case] sub: &str, #[case] expected: i64) {
        unsafe {
            let result_ptr = cel_index_of_poly(
                Box::into_raw(Box::new(CelValue::String(s.to_string()))),
                Box::into_raw(Box::new(CelValue::String(sub.to_string()))),
            );
            assert_eq!(&*result_ptr, &CelValue::Int(expected));
        }
    }

    #[test]
    fn test_index_of_poly_array_found() {
        unsafe {
            let result_ptr = cel_index_of_poly(
                Box::into_raw(Box::new(CelValue::Array(vec![
                    CelValue::Int(10),
                    CelValue::Int(20),
                    CelValue::Int(30),
                ]))),
                Box::into_raw(Box::new(CelValue::Int(20))),
            );
            assert_eq!(&*result_ptr, &CelValue::Int(1));
        }
    }

    #[test]
    fn test_index_of_poly_array_not_found() {
        unsafe {
            let result_ptr = cel_index_of_poly(
                Box::into_raw(Box::new(CelValue::Array(vec![
                    CelValue::Int(1),
                    CelValue::Int(2),
                ]))),
                Box::into_raw(Box::new(CelValue::Int(99))),
            );
            assert_eq!(&*result_ptr, &CelValue::Int(-1));
        }
    }

    #[rstest]
    #[case::bool_receiver(CelValue::Bool(true))]
    #[case::int_receiver(CelValue::Int(42))]
    fn test_index_of_poly_wrong_receiver_returns_error(#[case] receiver: CelValue) {
        unsafe {
            let result_ptr = cel_index_of_poly(
                Box::into_raw(Box::new(receiver)),
                Box::into_raw(Box::new(CelValue::Int(1))),
            );
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
        }
    }

    // ── cel_last_index_of_poly ────────────────────────────────────────────────

    #[rstest]
    #[case::string_found("tacocat", "at", 5_i64)]
    #[case::string_not_found("tacocat", "none", -1_i64)]
    #[case::string_empty_needle("tacocat", "", 7_i64)]
    fn test_last_index_of_poly_string(#[case] s: &str, #[case] sub: &str, #[case] expected: i64) {
        unsafe {
            let result_ptr = cel_last_index_of_poly(
                Box::into_raw(Box::new(CelValue::String(s.to_string()))),
                Box::into_raw(Box::new(CelValue::String(sub.to_string()))),
            );
            assert_eq!(&*result_ptr, &CelValue::Int(expected));
        }
    }

    #[test]
    fn test_last_index_of_poly_array_last_occurrence() {
        // [10, 20, 10] — last 10 is at index 2
        unsafe {
            let result_ptr = cel_last_index_of_poly(
                Box::into_raw(Box::new(CelValue::Array(vec![
                    CelValue::Int(10),
                    CelValue::Int(20),
                    CelValue::Int(10),
                ]))),
                Box::into_raw(Box::new(CelValue::Int(10))),
            );
            assert_eq!(&*result_ptr, &CelValue::Int(2));
        }
    }

    #[test]
    fn test_last_index_of_poly_array_not_found() {
        unsafe {
            let result_ptr = cel_last_index_of_poly(
                Box::into_raw(Box::new(CelValue::Array(vec![
                    CelValue::Int(1),
                    CelValue::Int(2),
                ]))),
                Box::into_raw(Box::new(CelValue::Int(99))),
            );
            assert_eq!(&*result_ptr, &CelValue::Int(-1));
        }
    }

    #[rstest]
    #[case::bool_receiver(CelValue::Bool(false))]
    #[case::int_receiver(CelValue::Int(0))]
    fn test_last_index_of_poly_wrong_receiver_returns_error(#[case] receiver: CelValue) {
        unsafe {
            let result_ptr = cel_last_index_of_poly(
                Box::into_raw(Box::new(receiver)),
                Box::into_raw(Box::new(CelValue::Int(1))),
            );
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
        }
    }

    // ── cel_reverse_poly ──────────────────────────────────────────────────────

    #[rstest]
    #[case::basic("gums", "smug")]
    #[case::empty("", "")]
    #[case::unicode("héllo", "olléh")]
    fn test_reverse_poly_string(#[case] input: &str, #[case] expected: &str) {
        unsafe {
            let result_ptr =
                cel_reverse_poly(Box::into_raw(Box::new(CelValue::String(input.to_string()))));
            assert_eq!(&*result_ptr, &CelValue::String(expected.to_string()));
        }
    }

    #[rstest]
    #[case::empty(vec![], vec![])]
    #[case::ints(
        vec![CelValue::Int(5), CelValue::Int(1), CelValue::Int(2)],
        vec![CelValue::Int(2), CelValue::Int(1), CelValue::Int(5)]
    )]
    fn test_reverse_poly_list(#[case] input: Vec<CelValue>, #[case] expected: Vec<CelValue>) {
        unsafe {
            let result_ptr = cel_reverse_poly(Box::into_raw(Box::new(CelValue::Array(input))));
            assert_eq!(&*result_ptr, &CelValue::Array(expected));
        }
    }

    #[test]
    fn test_reverse_poly_wrong_type_returns_error() {
        unsafe {
            let result_ptr = cel_reverse_poly(Box::into_raw(Box::new(CelValue::Int(42))));
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
        }
    }
}
