//! CEL extended list library functions.
//!
//! Implements list extension functions from the CEL extended library:
//!
//! - `join` / `join(sep)` — join a list of strings
//! - `distinct` — remove duplicate elements (first-occurrence order)
//! - `flatten` / `flatten(depth)` — flatten nested lists
//! - `lists.range(n)` — generate `[0, 1, ..., n-1]`
//! - `reverse` — reverse element order
//! - `slice(start, end)` — sub-list by index range
//! - `sort` — sort comparable elements

use crate::helpers::{cel_equals, cel_value_less_than};
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

/// Returns the distinct elements of a list, preserving first-occurrence order.
///
/// Uses `cel_equals` for element comparison (handles cross-type numeric equality).
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_list_distinct(list_ptr: *const CelValue) -> *mut CelValue {
    let list = match unsafe { &*list_ptr } {
        CelValue::Array(v) => v,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "distinct: receiver is not a list".to_string(),
            )));
        }
    };
    let mut unique: Vec<CelValue> = Vec::new();
    for val in list {
        if !unique.iter().any(|u| cel_equals(u, val)) {
            unique.push(val.clone());
        }
    }
    Box::into_raw(Box::new(CelValue::Array(unique)))
}

/// Flattens a list one level deep.
///
/// Non-list elements are kept as-is. Sub-lists are expanded into the output.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_list_flatten(list_ptr: *const CelValue) -> *mut CelValue {
    let list = match unsafe { &*list_ptr } {
        CelValue::Array(v) => v,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "flatten: receiver is not a list".to_string(),
            )));
        }
    };
    let result = list_flatten_depth(list, 1);
    Box::into_raw(Box::new(CelValue::Array(result)))
}

/// Flattens a list to the given depth. Depth must be non-negative.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_list_flatten_depth(
    list_ptr: *const CelValue,
    depth_ptr: *const CelValue,
) -> *mut CelValue {
    let list = match unsafe { &*list_ptr } {
        CelValue::Array(v) => v,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "flatten: receiver is not a list".to_string(),
            )));
        }
    };
    let depth = match unsafe { &*depth_ptr } {
        CelValue::Int(n) => *n,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "flatten: depth must be an int".to_string(),
            )));
        }
    };
    if depth < 0 {
        return Box::into_raw(Box::new(CelValue::Error(
            "level must be non-negative".to_string(),
        )));
    }
    let result = list_flatten_depth(list, depth);
    Box::into_raw(Box::new(CelValue::Array(result)))
}

/// Internal recursive flatten helper.
fn list_flatten_depth(list: &[CelValue], depth: i64) -> Vec<CelValue> {
    let mut result = Vec::new();
    for val in list {
        match val {
            CelValue::Array(inner) if depth > 0 => {
                result.extend(list_flatten_depth(inner, depth - 1));
            }
            _ => result.push(val.clone()),
        }
    }
    result
}

/// Returns a list of integers `[0, 1, ..., n-1]`.
///
/// If `n <= 0`, returns an empty list.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_list_range(n_ptr: *const CelValue) -> *mut CelValue {
    let n = match unsafe { &*n_ptr } {
        CelValue::Int(n) => *n,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "lists.range: argument must be an int".to_string(),
            )));
        }
    };
    let result: Vec<CelValue> = (0..n.max(0)).map(CelValue::Int).collect();
    Box::into_raw(Box::new(CelValue::Array(result)))
}

/// Returns the elements of a list in reverse order.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_list_reverse(list_ptr: *const CelValue) -> *mut CelValue {
    let list = match unsafe { &*list_ptr } {
        CelValue::Array(v) => v.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "reverse: receiver is not a list".to_string(),
            )));
        }
    };
    let mut reversed = list;
    reversed.reverse();
    Box::into_raw(Box::new(CelValue::Array(reversed)))
}

/// Returns a sub-list from `start` (inclusive) to `end` (exclusive).
///
/// Errors if:
/// - Either index is negative
/// - `start > end`
/// - `end > len`
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_list_slice(
    list_ptr: *const CelValue,
    start_ptr: *const CelValue,
    end_ptr: *const CelValue,
) -> *mut CelValue {
    let list = match unsafe { &*list_ptr } {
        CelValue::Array(v) => v,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "slice: receiver is not a list".to_string(),
            )));
        }
    };
    let start = match unsafe { &*start_ptr } {
        CelValue::Int(n) => *n,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "slice: start index must be an int".to_string(),
            )));
        }
    };
    let end = match unsafe { &*end_ptr } {
        CelValue::Int(n) => *n,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "slice: end index must be an int".to_string(),
            )));
        }
    };
    let len = list.len() as i64;
    if start < 0 || end < 0 {
        return Box::into_raw(Box::new(CelValue::Error(format!(
            "cannot slice({start}, {end}), negative indexes not supported"
        ))));
    }
    if start > end {
        return Box::into_raw(Box::new(CelValue::Error(format!(
            "cannot slice({start}, {end}), start index must be less than or equal to end index"
        ))));
    }
    if end > len {
        return Box::into_raw(Box::new(CelValue::Error(format!(
            "cannot slice({start}, {end}), list is length {len}"
        ))));
    }
    let result = list[start as usize..end as usize].to_vec();
    Box::into_raw(Box::new(CelValue::Array(result)))
}

/// Sorts a list of comparable elements in ascending order.
///
/// All elements must be of the same comparable type. Mixed types or incomparable
/// types produce an error.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_list_sort(list_ptr: *const CelValue) -> *mut CelValue {
    let list = match unsafe { &*list_ptr } {
        CelValue::Array(v) => v.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "sort: receiver is not a list".to_string(),
            )));
        }
    };
    if list.is_empty() {
        return Box::into_raw(Box::new(CelValue::Array(list)));
    }
    // Validate that all elements are the same type discriminant (comparable check via less_than)
    let first = &list[0];
    for elem in &list[1..] {
        if cel_value_less_than(first, elem).is_err() && cel_value_less_than(elem, first).is_err() {
            return Box::into_raw(Box::new(CelValue::Error(
                "list elements must be comparable".to_string(),
            )));
        }
    }
    let mut sorted = list;
    let mut sort_err: Option<String> = None;
    sorted.sort_by(|a, b| {
        if sort_err.is_some() {
            return std::cmp::Ordering::Equal;
        }
        match cel_value_less_than(a, b) {
            Ok(true) => std::cmp::Ordering::Less,
            Ok(false) => match cel_value_less_than(b, a) {
                Ok(true) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            },
            Err(_) => {
                sort_err = Some("list elements must have the same type".to_string());
                std::cmp::Ordering::Equal
            }
        }
    });
    if let Some(err) = sort_err {
        return Box::into_raw(Box::new(CelValue::Error(err)));
    }
    Box::into_raw(Box::new(CelValue::Array(sorted)))
}

/// Returns an `optional` containing the first element of the list, or `optional.none()` if empty.
///
/// # Safety
///
/// Caller must ensure the pointer argument points to a valid `CelValue` instance
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_list_first(list_ptr: *const CelValue) -> *mut CelValue {
    let list = match unsafe { &*list_ptr } {
        CelValue::Array(v) => v,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "first: receiver is not a list".to_string(),
            )));
        }
    };
    if list.is_empty() {
        return Box::into_raw(Box::new(CelValue::Optional(None)));
    }
    Box::into_raw(Box::new(CelValue::Optional(Some(Box::new(
        list[0].clone(),
    )))))
}

/// Returns an `optional` containing the last element of the list, or `optional.none()` if empty.
///
/// # Safety
///
/// Caller must ensure the pointer argument points to a valid `CelValue` instance
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_list_last(list_ptr: *const CelValue) -> *mut CelValue {
    let list = match unsafe { &*list_ptr } {
        CelValue::Array(v) => v,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "last: receiver is not a list".to_string(),
            )));
        }
    };
    if list.is_empty() {
        return Box::into_raw(Box::new(CelValue::Optional(None)));
    }
    Box::into_raw(Box::new(CelValue::Optional(Some(Box::new(
        list[list.len() - 1].clone(),
    )))))
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

    // ── distinct ──────────────────────────────────────────────────────────────

    #[rstest]
    #[case::empty(vec![], vec![])]
    #[case::no_dups(vec![CelValue::Int(1), CelValue::Int(2)], vec![CelValue::Int(1), CelValue::Int(2)])]
    #[case::ints(
        vec![CelValue::Int(-2), CelValue::Int(5), CelValue::Int(-2), CelValue::Int(1), CelValue::Int(1), CelValue::Int(5), CelValue::Int(-2), CelValue::Int(1)],
        vec![CelValue::Int(-2), CelValue::Int(5), CelValue::Int(1)]
    )]
    #[case::strings(
        vec![CelValue::String("c".into()), CelValue::String("a".into()), CelValue::String("a".into()), CelValue::String("b".into())],
        vec![CelValue::String("c".into()), CelValue::String("a".into()), CelValue::String("b".into())]
    )]
    #[case::cross_type_numeric(
        vec![CelValue::Int(1), CelValue::Double(1.0), CelValue::Int(2)],
        vec![CelValue::Int(1), CelValue::Int(2)]
    )]
    fn test_list_distinct(#[case] input: Vec<CelValue>, #[case] expected: Vec<CelValue>) {
        let list = CelValue::Array(input);
        unsafe {
            let result_ptr = cel_list_distinct(&list as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::Array(expected));
            cel_free_value(result_ptr);
        }
    }

    // ── flatten ───────────────────────────────────────────────────────────────

    #[rstest]
    #[case::empty(vec![], 1, vec![])]
    #[case::already_flat(vec![CelValue::Int(1), CelValue::Int(2)], 1, vec![CelValue::Int(1), CelValue::Int(2)])]
    #[case::one_level(
        vec![CelValue::Int(1), CelValue::Array(vec![CelValue::Int(2), CelValue::Int(3)]), CelValue::Array(vec![CelValue::Int(4)])],
        1,
        vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3), CelValue::Int(4)]
    )]
    #[case::nested_default(
        vec![CelValue::Int(1), CelValue::Array(vec![CelValue::Int(2), CelValue::Array(vec![CelValue::Int(3), CelValue::Int(4)])])],
        1,
        vec![CelValue::Int(1), CelValue::Int(2), CelValue::Array(vec![CelValue::Int(3), CelValue::Int(4)])]
    )]
    #[case::depth_two(
        vec![CelValue::Int(1), CelValue::Array(vec![CelValue::Int(2), CelValue::Array(vec![CelValue::Int(3), CelValue::Int(4)])])],
        2,
        vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3), CelValue::Int(4)]
    )]
    #[case::empty_sublists(
        vec![CelValue::Int(1), CelValue::Int(2), CelValue::Array(vec![]), CelValue::Array(vec![]), CelValue::Array(vec![CelValue::Int(3), CelValue::Int(4)])],
        1,
        vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3), CelValue::Int(4)]
    )]
    fn test_list_flatten_depth(
        #[case] input: Vec<CelValue>,
        #[case] depth: i64,
        #[case] expected: Vec<CelValue>,
    ) {
        let list = CelValue::Array(input);
        let depth_val = CelValue::Int(depth);
        unsafe {
            let result_ptr =
                cel_list_flatten_depth(&list as *const CelValue, &depth_val as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::Array(expected));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_list_flatten_default_is_depth_one() {
        let list = CelValue::Array(vec![
            CelValue::Int(1),
            CelValue::Array(vec![
                CelValue::Int(2),
                CelValue::Array(vec![CelValue::Int(3), CelValue::Int(4)]),
            ]),
        ]);
        unsafe {
            let result_ptr = cel_list_flatten(&list as *const CelValue);
            assert_eq!(
                &*result_ptr,
                &CelValue::Array(vec![
                    CelValue::Int(1),
                    CelValue::Int(2),
                    CelValue::Array(vec![CelValue::Int(3), CelValue::Int(4)]),
                ])
            );
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_list_flatten_depth_negative_returns_error() {
        let list = CelValue::Array(vec![]);
        let depth_val = CelValue::Int(-1);
        unsafe {
            let result_ptr =
                cel_list_flatten_depth(&list as *const CelValue, &depth_val as *const CelValue);
            assert!(matches!(&*result_ptr, CelValue::Error(e) if e.contains("non-negative")));
            cel_free_value(result_ptr);
        }
    }

    // ── range ─────────────────────────────────────────────────────────────────

    #[rstest]
    #[case::zero(0, vec![])]
    #[case::negative(-1, vec![])]
    #[case::one(1, vec![CelValue::Int(0)])]
    #[case::four(4, vec![CelValue::Int(0), CelValue::Int(1), CelValue::Int(2), CelValue::Int(3)])]
    fn test_list_range(#[case] n: i64, #[case] expected: Vec<CelValue>) {
        let n_val = CelValue::Int(n);
        unsafe {
            let result_ptr = cel_list_range(&n_val as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::Array(expected));
            cel_free_value(result_ptr);
        }
    }

    // ── reverse ───────────────────────────────────────────────────────────────

    #[rstest]
    #[case::empty(vec![], vec![])]
    #[case::single(vec![CelValue::Int(1)], vec![CelValue::Int(1)])]
    #[case::multiple(
        vec![CelValue::Int(5), CelValue::Int(1), CelValue::Int(2), CelValue::Int(3)],
        vec![CelValue::Int(3), CelValue::Int(2), CelValue::Int(1), CelValue::Int(5)]
    )]
    #[case::strings(
        vec![CelValue::String("are".into()), CelValue::String("you".into()), CelValue::String("am".into())],
        vec![CelValue::String("am".into()), CelValue::String("you".into()), CelValue::String("are".into())]
    )]
    fn test_list_reverse(#[case] input: Vec<CelValue>, #[case] expected: Vec<CelValue>) {
        let list = CelValue::Array(input);
        unsafe {
            let result_ptr = cel_list_reverse(&list as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::Array(expected));
            cel_free_value(result_ptr);
        }
    }

    // ── slice ─────────────────────────────────────────────────────────────────

    #[rstest]
    #[case::full(0, 4, vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3), CelValue::Int(4)])]
    #[case::empty_from_start(0, 0, vec![])]
    #[case::empty_mid(1, 1, vec![])]
    #[case::empty_end(4, 4, vec![])]
    #[case::middle(1, 3, vec![CelValue::Int(2), CelValue::Int(3)])]
    #[case::tail(2, 4, vec![CelValue::Int(3), CelValue::Int(4)])]
    fn test_list_slice(#[case] start: i64, #[case] end: i64, #[case] expected: Vec<CelValue>) {
        let list = CelValue::Array(vec![
            CelValue::Int(1),
            CelValue::Int(2),
            CelValue::Int(3),
            CelValue::Int(4),
        ]);
        let start_val = CelValue::Int(start);
        let end_val = CelValue::Int(end);
        unsafe {
            let result_ptr = cel_list_slice(
                &list as *const CelValue,
                &start_val as *const CelValue,
                &end_val as *const CelValue,
            );
            assert_eq!(&*result_ptr, &CelValue::Array(expected));
            cel_free_value(result_ptr);
        }
    }

    #[rstest]
    #[case::start_after_end(3, 0, "start index must be less than or equal to end index")]
    #[case::end_out_of_bounds(0, 10, "list is length 4")]
    #[case::negative_start(-5, 10, "negative indexes not supported")]
    #[case::both_negative(-5, -3, "negative indexes not supported")]
    fn test_list_slice_errors(#[case] start: i64, #[case] end: i64, #[case] msg: &str) {
        let list = CelValue::Array(vec![
            CelValue::Int(1),
            CelValue::Int(2),
            CelValue::Int(3),
            CelValue::Int(4),
        ]);
        let start_val = CelValue::Int(start);
        let end_val = CelValue::Int(end);
        unsafe {
            let result_ptr = cel_list_slice(
                &list as *const CelValue,
                &start_val as *const CelValue,
                &end_val as *const CelValue,
            );
            assert!(
                matches!(&*result_ptr, CelValue::Error(e) if e.contains(msg)),
                "expected error containing {:?}, got {:?}",
                msg,
                &*result_ptr
            );
            cel_free_value(result_ptr);
        }
    }

    // ── sort ──────────────────────────────────────────────────────────────────

    #[rstest]
    #[case::empty(vec![], vec![])]
    #[case::single(vec![CelValue::Int(1)], vec![CelValue::Int(1)])]
    #[case::ints(
        vec![CelValue::Int(4), CelValue::Int(3), CelValue::Int(2), CelValue::Int(1)],
        vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3), CelValue::Int(4)]
    )]
    #[case::strings(
        vec![CelValue::String("d".into()), CelValue::String("a".into()), CelValue::String("b".into()), CelValue::String("c".into())],
        vec![CelValue::String("a".into()), CelValue::String("b".into()), CelValue::String("c".into()), CelValue::String("d".into())]
    )]
    #[case::bools(
        vec![CelValue::Bool(true), CelValue::Bool(false), CelValue::Bool(true)],
        vec![CelValue::Bool(false), CelValue::Bool(true), CelValue::Bool(true)]
    )]
    fn test_list_sort(#[case] input: Vec<CelValue>, #[case] expected: Vec<CelValue>) {
        let list = CelValue::Array(input);
        unsafe {
            let result_ptr = cel_list_sort(&list as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::Array(expected));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_list_sort_mixed_types_returns_error() {
        let list = CelValue::Array(vec![CelValue::String("d".into()), CelValue::Int(3)]);
        unsafe {
            let result_ptr = cel_list_sort(&list as *const CelValue);
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }

    // ── first ─────────────────────────────────────────────────────────────────

    #[rstest]
    #[case::empty(vec![], None)]
    #[case::single_int(vec![CelValue::Int(42)], Some(CelValue::Int(42)))]
    #[case::ints(
        vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3)],
        Some(CelValue::Int(1))
    )]
    #[case::strings(
        vec![CelValue::String("a".into()), CelValue::String("b".into()), CelValue::String("c".into())],
        Some(CelValue::String("a".into()))
    )]
    #[case::bools(
        vec![CelValue::Bool(true), CelValue::Bool(false)],
        Some(CelValue::Bool(true))
    )]
    #[case::doubles(
        vec![CelValue::Double(1.5), CelValue::Double(2.5)],
        Some(CelValue::Double(1.5))
    )]
    fn test_list_first(#[case] input: Vec<CelValue>, #[case] expected_inner: Option<CelValue>) {
        let list = CelValue::Array(input);
        unsafe {
            let result_ptr = cel_list_first(&list as *const CelValue);
            let expected = CelValue::Optional(expected_inner.map(Box::new));
            assert_eq!(&*result_ptr, &expected);
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_list_first_non_list_returns_error() {
        let val = CelValue::Int(42);
        unsafe {
            let result_ptr = cel_list_first(&val as *const CelValue);
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }

    // ── last ──────────────────────────────────────────────────────────────────

    #[rstest]
    #[case::empty(vec![], None)]
    #[case::single_int(vec![CelValue::Int(42)], Some(CelValue::Int(42)))]
    #[case::ints(
        vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3)],
        Some(CelValue::Int(3))
    )]
    #[case::strings(
        vec![CelValue::String("a".into()), CelValue::String("b".into()), CelValue::String("c".into())],
        Some(CelValue::String("c".into()))
    )]
    #[case::bools(
        vec![CelValue::Bool(true), CelValue::Bool(false)],
        Some(CelValue::Bool(false))
    )]
    #[case::doubles(
        vec![CelValue::Double(1.5), CelValue::Double(2.5)],
        Some(CelValue::Double(2.5))
    )]
    fn test_list_last(#[case] input: Vec<CelValue>, #[case] expected_inner: Option<CelValue>) {
        let list = CelValue::Array(input);
        unsafe {
            let result_ptr = cel_list_last(&list as *const CelValue);
            let expected = CelValue::Optional(expected_inner.map(Box::new));
            assert_eq!(&*result_ptr, &expected);
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_list_last_non_list_returns_error() {
        let val = CelValue::Int(42);
        unsafe {
            let result_ptr = cel_list_last(&val as *const CelValue);
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }
}
