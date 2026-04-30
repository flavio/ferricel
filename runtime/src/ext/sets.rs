//! CEL sets extension library functions.
//!
//! Implements the three set-relationship functions from the CEL extended library:
//!
//! - `sets.contains(list, sublist) -> bool`  — every element in `sublist` exists in `list`
//! - `sets.intersects(listA, listB) -> bool` — at least one element is shared between the lists
//! - `sets.equivalent(listA, listB) -> bool` — both lists contain the same elements (as sets)
//!
//! All functions use CEL equality (`cel_equals`) for element comparison, which handles
//! cross-type numeric equality (`1 == 1u == 1.0`) and recursive list/map comparison.
//!
//! The algorithms mirror the Go reference implementation (O(n*m) linear scans).

use crate::error::read_ptr;
use crate::helpers::cel_equals;
use crate::types::CelValue;

/// Check whether every element of `sublist` exists somewhere in `list`.
///
/// Returns `true` if `sublist` is empty.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_sets_contains(
    list_ptr: *mut CelValue,
    sublist_ptr: *mut CelValue,
) -> *mut CelValue {
    let list_val = unsafe { read_ptr(list_ptr) };
    let sublist_val = unsafe { read_ptr(sublist_ptr) };
    let list = match list_val {
        CelValue::Array(v) => v,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "sets.contains: first argument is not a list".to_string(),
            )));
        }
    };
    let sublist = match sublist_val {
        CelValue::Array(v) => v,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "sets.contains: second argument is not a list".to_string(),
            )));
        }
    };

    let result = sets_contains(&list, &sublist);
    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Check whether `listA` and `listB` share at least one element.
///
/// Returns `false` if either list is empty.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_sets_intersects(
    list_a_ptr: *mut CelValue,
    list_b_ptr: *mut CelValue,
) -> *mut CelValue {
    let list_a_val = unsafe { read_ptr(list_a_ptr) };
    let list_b_val = unsafe { read_ptr(list_b_ptr) };
    let list_a = match list_a_val {
        CelValue::Array(v) => v,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "sets.intersects: first argument is not a list".to_string(),
            )));
        }
    };
    let list_b = match list_b_val {
        CelValue::Array(v) => v,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "sets.intersects: second argument is not a list".to_string(),
            )));
        }
    };

    let result = list_a
        .iter()
        .any(|a| list_b.iter().any(|b| cel_equals(a, b)));
    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Check whether `listA` and `listB` are set-equivalent.
///
/// Two lists are set-equivalent when every element in `listA` exists in `listB` and
/// every element in `listB` exists in `listA`. Duplicates and order are ignored.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_sets_equivalent(
    list_a_ptr: *mut CelValue,
    list_b_ptr: *mut CelValue,
) -> *mut CelValue {
    let list_a_val = unsafe { read_ptr(list_a_ptr) };
    let list_b_val = unsafe { read_ptr(list_b_ptr) };
    let list_a = match list_a_val {
        CelValue::Array(v) => v,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "sets.equivalent: first argument is not a list".to_string(),
            )));
        }
    };
    let list_b = match list_b_val {
        CelValue::Array(v) => v,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "sets.equivalent: second argument is not a list".to_string(),
            )));
        }
    };

    let result = sets_contains(&list_a, &list_b) && sets_contains(&list_b, &list_a);
    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Internal helper: returns `true` if every element of `sublist` exists in `list`.
///
/// Uses `cel_equals` for element comparison (cross-type numeric equality).
fn sets_contains(list: &[CelValue], sublist: &[CelValue]) -> bool {
    sublist
        .iter()
        .all(|sub_elem| list.iter().any(|list_elem| cel_equals(list_elem, sub_elem)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // --- sets.contains ---

    #[rstest]
    #[case::both_empty(vec![], vec![], true)]
    #[case::empty_sublist(vec![CelValue::Int(1)], vec![], true)]
    #[case::single_match(vec![CelValue::Int(1)], vec![CelValue::Int(1)], true)]
    #[case::dup_in_sublist(vec![CelValue::Int(1)], vec![CelValue::Int(1), CelValue::Int(1)], true)]
    #[case::dup_in_list(vec![CelValue::Int(1), CelValue::Int(1)], vec![CelValue::Int(1)], true)]
    #[case::reordered(vec![CelValue::Int(2), CelValue::Int(1)], vec![CelValue::Int(1)], true)]
    #[case::subset(
        vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3), CelValue::Int(4)],
        vec![CelValue::Int(2), CelValue::Int(3)],
        true
    )]
    #[case::int_double(
        vec![CelValue::Int(1)],
        vec![CelValue::Double(1.0), CelValue::Int(1)],
        true
    )]
    #[case::int_uint_double(
        vec![CelValue::Int(1), CelValue::Int(2)],
        vec![CelValue::UInt(2), CelValue::Double(2.0)],
        true
    )]
    #[case::uint_in_int_list(
        vec![CelValue::Int(1), CelValue::UInt(2)],
        vec![CelValue::Int(2), CelValue::Double(2.0)],
        true
    )]
    #[case::mixed_numeric(
        vec![CelValue::Int(1), CelValue::Double(2.0), CelValue::UInt(3)],
        vec![CelValue::Double(1.0), CelValue::UInt(2), CelValue::Int(3)],
        true
    )]
    #[case::not_found(vec![CelValue::Int(1)], vec![CelValue::Int(2)], false)]
    #[case::partial_miss(
        vec![CelValue::Int(1)],
        vec![CelValue::Int(1), CelValue::Int(2)],
        false
    )]
    #[case::type_mismatch(
        vec![CelValue::Int(1)],
        vec![CelValue::String("1".to_string()), CelValue::Int(1)],
        false
    )]
    #[case::close_but_no(
        vec![CelValue::Int(1)],
        vec![CelValue::Double(1.1), CelValue::UInt(1)],
        false
    )]
    fn test_sets_contains(
        #[case] list: Vec<CelValue>,
        #[case] sublist: Vec<CelValue>,
        #[case] expected: bool,
    ) {
        let list_ptr = Box::into_raw(Box::new(CelValue::Array(list)));
        let sublist_ptr = Box::into_raw(Box::new(CelValue::Array(sublist)));
        unsafe {
            let result_ptr = cel_sets_contains(list_ptr, sublist_ptr);
            let result = match &*result_ptr {
                CelValue::Bool(b) => *b,
                other => panic!("Expected Bool, got {:?}", other),
            };
            assert_eq!(result, expected);
            let _ = Box::from_raw(result_ptr);
        }
    }

    // --- sets.intersects ---

    #[rstest]
    #[case::both_empty(vec![], vec![], false)]
    #[case::empty_second(vec![CelValue::Int(1)], vec![], false)]
    #[case::single_match(vec![CelValue::Int(1)], vec![CelValue::Int(1)], true)]
    #[case::dup_in_second(vec![CelValue::Int(1)], vec![CelValue::Int(1), CelValue::Int(1)], true)]
    #[case::dup_in_first(vec![CelValue::Int(1), CelValue::Int(1)], vec![CelValue::Int(1)], true)]
    #[case::reordered(vec![CelValue::Int(2), CelValue::Int(1)], vec![CelValue::Int(1)], true)]
    #[case::partial(
        vec![CelValue::Int(1)],
        vec![CelValue::Int(1), CelValue::Int(2)],
        true
    )]
    #[case::int_double(
        vec![CelValue::Int(1)],
        vec![CelValue::Double(1.0), CelValue::Int(2)],
        true
    )]
    #[case::mixed_numeric_a(
        vec![CelValue::Int(1), CelValue::Int(2)],
        vec![CelValue::UInt(2), CelValue::Int(2), CelValue::Double(2.0)],
        true
    )]
    #[case::mixed_numeric_b(
        vec![CelValue::Int(1), CelValue::Int(2)],
        vec![CelValue::UInt(1), CelValue::Int(2), CelValue::Double(2.3)],
        true
    )]
    #[case::no_match(vec![CelValue::Int(1)], vec![CelValue::Int(2)], false)]
    #[case::type_mismatch(
        vec![CelValue::Int(1)],
        vec![CelValue::String("1".to_string()), CelValue::Int(2)],
        false
    )]
    #[case::close_but_no(
        vec![CelValue::Int(1)],
        vec![CelValue::Double(1.1), CelValue::UInt(2)],
        false
    )]
    fn test_sets_intersects(
        #[case] list_a: Vec<CelValue>,
        #[case] list_b: Vec<CelValue>,
        #[case] expected: bool,
    ) {
        let a_ptr = Box::into_raw(Box::new(CelValue::Array(list_a)));
        let b_ptr = Box::into_raw(Box::new(CelValue::Array(list_b)));
        unsafe {
            let result_ptr = cel_sets_intersects(a_ptr, b_ptr);
            let result = match &*result_ptr {
                CelValue::Bool(b) => *b,
                other => panic!("Expected Bool, got {:?}", other),
            };
            assert_eq!(result, expected);
            let _ = Box::from_raw(result_ptr);
        }
    }

    // --- sets.equivalent ---

    #[rstest]
    #[case::both_empty(vec![], vec![], true)]
    #[case::single_match(vec![CelValue::Int(1)], vec![CelValue::Int(1)], true)]
    #[case::dup_in_second(vec![CelValue::Int(1)], vec![CelValue::Int(1), CelValue::Int(1)], true)]
    #[case::dup_in_first(vec![CelValue::Int(1), CelValue::Int(1)], vec![CelValue::Int(1)], true)]
    #[case::int_uint_double(
        vec![CelValue::Int(1)],
        vec![CelValue::UInt(1), CelValue::Double(1.0)],
        true
    )]
    #[case::reordered_mixed(
        vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3)],
        vec![CelValue::UInt(3), CelValue::Double(2.0), CelValue::Int(1)],
        true
    )]
    #[case::superset_not_equiv(
        vec![CelValue::Int(2), CelValue::Int(1)],
        vec![CelValue::Int(1)],
        false
    )]
    #[case::subset_not_equiv(
        vec![CelValue::Int(1)],
        vec![CelValue::Int(1), CelValue::Int(2)],
        false
    )]
    #[case::numeric_mismatch_a(
        vec![CelValue::Int(1), CelValue::Int(2)],
        vec![CelValue::UInt(2), CelValue::Int(2), CelValue::Double(2.0)],
        false
    )]
    #[case::numeric_mismatch_b(
        vec![CelValue::Int(1), CelValue::Int(2)],
        vec![CelValue::UInt(1), CelValue::Int(2), CelValue::Double(2.3)],
        false
    )]
    fn test_sets_equivalent(
        #[case] list_a: Vec<CelValue>,
        #[case] list_b: Vec<CelValue>,
        #[case] expected: bool,
    ) {
        let a_ptr = Box::into_raw(Box::new(CelValue::Array(list_a)));
        let b_ptr = Box::into_raw(Box::new(CelValue::Array(list_b)));
        unsafe {
            let result_ptr = cel_sets_equivalent(a_ptr, b_ptr);
            let result = match &*result_ptr {
                CelValue::Bool(b) => *b,
                other => panic!("Expected Bool, got {:?}", other),
            };
            assert_eq!(result, expected);
            let _ = Box::from_raw(result_ptr);
        }
    }
}
