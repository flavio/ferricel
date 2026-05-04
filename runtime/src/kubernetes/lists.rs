//! Kubernetes CEL list library extensions.
//!
//! Implements the additional list methods that Kubernetes adds to CEL:
//!   - `isSorted`     — true if the list is in non-descending order
//!   - `sum`          — sum of all elements (int/uint/double/duration)
//!   - `min`          — minimum element (error on empty list)
//!   - `max`          — maximum element (error on empty list)
//!   - `indexOf`      — first index of an element, or -1
//!   - `lastIndexOf`  — last index of an element, or -1
//!
//! Reference: <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-list-library>

use slog::error;

use crate::{
    error::{create_error_value, read_ptr},
    helpers::{cel_equals, cel_value_less_than},
    types::CelValue,
};

// ──────────────────────────────────────────────────────────────────────────────
// isSorted
// ──────────────────────────────────────────────────────────────────────────────

/// Returns true if the list elements are in non-descending (sorted) order.
///
/// An empty list or a single-element list is always considered sorted.
/// Supports all comparable CEL types: int, uint, double, string, bool,
/// bytes, timestamp, duration.
///
/// # Safety
/// `array_ptr` must be a valid, non-null pointer to a `CelValue::Array`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_list_is_sorted(array_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if array_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_list_is_sorted");
        return create_error_value("no such overload");
    }

    let array_value = unsafe { read_ptr(array_ptr) };

    match array_value {
        CelValue::Array(vec) => {
            // A list of 0 or 1 elements is trivially sorted.
            if vec.len() < 2 {
                return Box::into_raw(Box::new(CelValue::Bool(true)));
            }

            for window in vec.windows(2) {
                let (a, b) = (&window[0], &window[1]);
                // The list is sorted iff every adjacent pair satisfies a <= b,
                // i.e., NOT (b < a).
                match cel_value_less_than(b, a) {
                    Ok(b_lt_a) => {
                        if b_lt_a {
                            return Box::into_raw(Box::new(CelValue::Bool(false)));
                        }
                    }
                    Err(_) => {
                        error!(log, "incomparable types in isSorted";
                            "function" => "cel_k8s_list_is_sorted",
                            "left" => format!("{:?}", a),
                            "right" => format!("{:?}", b));
                        return create_error_value("no such overload");
                    }
                }
            }
            Box::into_raw(Box::new(CelValue::Bool(true)))
        }
        other => {
            error!(log, "expected Array";
                "function" => "cel_k8s_list_is_sorted",
                "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// sum
// ──────────────────────────────────────────────────────────────────────────────

/// Returns the sum of all elements in the list.
///
/// Supports: int, uint, double, duration.
/// An empty list returns `Int(0)` (per the Kubernetes CEL spec).
///
/// # Safety
/// `array_ptr` must be a valid, non-null pointer to a `CelValue::Array`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_list_sum(array_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if array_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_list_sum");
        return create_error_value("no such overload");
    }

    let array_value = unsafe { read_ptr(array_ptr) };

    match array_value {
        CelValue::Array(vec) => {
            if vec.is_empty() {
                // Spec: [].sum() returns 0
                return Box::into_raw(Box::new(CelValue::Int(0)));
            }

            // Determine the accumulator type from the first element and sum.
            match &vec[0] {
                CelValue::Int(_) => {
                    let mut acc: i64 = 0;
                    for elem in &vec {
                        match elem {
                            CelValue::Int(v) => {
                                acc = match acc.checked_add(*v) {
                                    Some(s) => s,
                                    None => {
                                        return create_error_value("integer overflow in sum");
                                    }
                                };
                            }
                            other => {
                                error!(log, "mixed types in sum";
                                    "function" => "cel_k8s_list_sum",
                                    "expected" => "Int",
                                    "got" => format!("{:?}", other));
                                return create_error_value("no such overload");
                            }
                        }
                    }
                    Box::into_raw(Box::new(CelValue::Int(acc)))
                }
                CelValue::UInt(_) => {
                    let mut acc: u64 = 0;
                    for elem in &vec {
                        match elem {
                            CelValue::UInt(v) => {
                                acc = match acc.checked_add(*v) {
                                    Some(s) => s,
                                    None => {
                                        return create_error_value("integer overflow in sum");
                                    }
                                };
                            }
                            other => {
                                error!(log, "mixed types in sum";
                                    "function" => "cel_k8s_list_sum",
                                    "expected" => "UInt",
                                    "got" => format!("{:?}", other));
                                return create_error_value("no such overload");
                            }
                        }
                    }
                    Box::into_raw(Box::new(CelValue::UInt(acc)))
                }
                CelValue::Double(_) => {
                    let mut acc: f64 = 0.0;
                    for elem in &vec {
                        match elem {
                            CelValue::Double(v) => acc += v,
                            other => {
                                error!(log, "mixed types in sum";
                                    "function" => "cel_k8s_list_sum",
                                    "expected" => "Double",
                                    "got" => format!("{:?}", other));
                                return create_error_value("no such overload");
                            }
                        }
                    }
                    Box::into_raw(Box::new(CelValue::Double(acc)))
                }
                CelValue::Duration(_) => {
                    let mut acc = chrono::Duration::zero();
                    for elem in &vec {
                        match elem {
                            CelValue::Duration(d) => {
                                acc = match acc.checked_add(d) {
                                    Some(s) => s,
                                    None => {
                                        return create_error_value("duration overflow in sum");
                                    }
                                };
                            }
                            other => {
                                error!(log, "mixed types in sum";
                                    "function" => "cel_k8s_list_sum",
                                    "expected" => "Duration",
                                    "got" => format!("{:?}", other));
                                return create_error_value("no such overload");
                            }
                        }
                    }
                    Box::into_raw(Box::new(CelValue::Duration(acc)))
                }
                other => {
                    error!(log, "unsupported element type for sum";
                        "function" => "cel_k8s_list_sum",
                        "got" => format!("{:?}", other));
                    create_error_value("no such overload")
                }
            }
        }
        other => {
            error!(log, "expected Array";
                "function" => "cel_k8s_list_sum",
                "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// min
// ──────────────────────────────────────────────────────────────────────────────

/// Returns the minimum element of the list.
///
/// Returns an error value if the list is empty.
/// Supports all comparable CEL types.
///
/// # Safety
/// `array_ptr` must be a valid, non-null pointer to a `CelValue::Array`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_list_min(array_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if array_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_list_min");
        return create_error_value("no such overload");
    }

    let array_value = unsafe { read_ptr(array_ptr) };

    match array_value {
        CelValue::Array(vec) => {
            if vec.is_empty() {
                return create_error_value("min called on empty list");
            }

            let mut min_idx = 0usize;
            for i in 1..vec.len() {
                match cel_value_less_than(&vec[i], &vec[min_idx]) {
                    Ok(elem_lt_min) => {
                        if elem_lt_min {
                            min_idx = i;
                        }
                    }
                    Err(_) => {
                        error!(log, "incomparable types in min";
                            "function" => "cel_k8s_list_min");
                        return create_error_value("no such overload");
                    }
                }
            }
            Box::into_raw(Box::new(vec.into_iter().nth(min_idx).unwrap()))
        }
        other => {
            error!(log, "expected Array";
                "function" => "cel_k8s_list_min",
                "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// max
// ──────────────────────────────────────────────────────────────────────────────

/// Returns the maximum element of the list.
///
/// Returns an error value if the list is empty.
/// Supports all comparable CEL types.
///
/// # Safety
/// `array_ptr` must be a valid, non-null pointer to a `CelValue::Array`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_list_max(array_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if array_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_list_max");
        return create_error_value("no such overload");
    }

    let array_value = unsafe { read_ptr(array_ptr) };

    match array_value {
        CelValue::Array(vec) => {
            if vec.is_empty() {
                return create_error_value("max called on empty list");
            }

            let mut max_idx = 0usize;
            for i in 1..vec.len() {
                // elem > max  iff  max < elem
                match cel_value_less_than(&vec[max_idx], &vec[i]) {
                    Ok(max_lt_elem) => {
                        if max_lt_elem {
                            max_idx = i;
                        }
                    }
                    Err(_) => {
                        error!(log, "incomparable types in max";
                            "function" => "cel_k8s_list_max");
                        return create_error_value("no such overload");
                    }
                }
            }
            Box::into_raw(Box::new(vec.into_iter().nth(max_idx).unwrap()))
        }
        other => {
            error!(log, "expected Array";
                "function" => "cel_k8s_list_max",
                "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// indexOf
// ──────────────────────────────────────────────────────────────────────────────

/// Returns the first (lowest) index at which `elem` appears in the list,
/// or -1 if the element is not found.
///
/// # Safety
/// Both `array_ptr` and `elem_ptr` must be valid, non-null pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_list_index_of(
    array_ptr: *mut CelValue,
    elem_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if array_ptr.is_null() || elem_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_list_index_of");
        return create_error_value("no such overload");
    }

    let array_value = unsafe { read_ptr(array_ptr) };
    let needle = unsafe { read_ptr(elem_ptr) };

    match array_value {
        CelValue::Array(vec) => {
            for (i, elem) in vec.iter().enumerate() {
                if cel_equals(elem, &needle) {
                    return Box::into_raw(Box::new(CelValue::Int(i as i64)));
                }
            }
            Box::into_raw(Box::new(CelValue::Int(-1)))
        }
        other => {
            error!(log, "expected Array";
                "function" => "cel_k8s_list_index_of",
                "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// lastIndexOf
// ──────────────────────────────────────────────────────────────────────────────

/// Returns the last (highest) index at which `elem` appears in the list,
/// or -1 if the element is not found.
///
/// # Safety
/// Both `array_ptr` and `elem_ptr` must be valid, non-null pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_list_last_index_of(
    array_ptr: *mut CelValue,
    elem_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if array_ptr.is_null() || elem_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_list_last_index_of");
        return create_error_value("no such overload");
    }

    let array_value = unsafe { read_ptr(array_ptr) };
    let needle = unsafe { read_ptr(elem_ptr) };

    match array_value {
        CelValue::Array(vec) => {
            for (i, elem) in vec.iter().enumerate().rev() {
                if cel_equals(elem, &needle) {
                    return Box::into_raw(Box::new(CelValue::Int(i as i64)));
                }
            }
            Box::into_raw(Box::new(CelValue::Int(-1)))
        }
        other => {
            error!(log, "expected Array";
                "function" => "cel_k8s_list_last_index_of",
                "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{
        super::test_helpers::{make_array, make_val, read_val},
        *,
    };

    // ── isSorted ────────────────────────────────────────────────────────────

    #[rstest]
    #[case::empty(vec![], CelValue::Bool(true))]
    #[case::single(vec![CelValue::Int(5)], CelValue::Bool(true))]
    #[case::ints_ascending(vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3)], CelValue::Bool(true))]
    #[case::ints_equal_adjacent(vec![CelValue::Int(1), CelValue::Int(1), CelValue::Int(2)], CelValue::Bool(true))]
    #[case::ints_descending(vec![CelValue::Int(3), CelValue::Int(1)], CelValue::Bool(false))]
    #[case::strings_sorted(
        vec![
            CelValue::String("a".to_string()),
            CelValue::String("b".to_string()),
            CelValue::String("b".to_string()),
            CelValue::String("c".to_string()),
        ],
        CelValue::Bool(true)
    )]
    #[case::doubles_descending(vec![CelValue::Double(2.0), CelValue::Double(1.0)], CelValue::Bool(false))]
    fn test_is_sorted(#[case] elements: Vec<CelValue>, #[case] expected: CelValue) {
        let arr = make_array(elements);
        let result = unsafe { read_val(cel_k8s_list_is_sorted(arr)) };
        assert_eq!(result, expected);
    }

    // ── sum ─────────────────────────────────────────────────────────────────

    #[rstest]
    #[case::empty(vec![], CelValue::Int(0))]
    #[case::ints(vec![CelValue::Int(1), CelValue::Int(3)], CelValue::Int(4))]
    #[case::doubles(vec![CelValue::Double(1.0), CelValue::Double(3.0)], CelValue::Double(4.0))]
    #[case::uints(vec![CelValue::UInt(2), CelValue::UInt(5)], CelValue::UInt(7))]
    #[case::durations(
        vec![
            CelValue::Duration(chrono::Duration::seconds(60)),
            CelValue::Duration(chrono::Duration::seconds(1)),
        ],
        CelValue::Duration(chrono::Duration::seconds(61))
    )]
    fn test_sum(#[case] elements: Vec<CelValue>, #[case] expected: CelValue) {
        let arr = make_array(elements);
        let result = unsafe { read_val(cel_k8s_list_sum(arr)) };
        assert_eq!(result, expected);
    }

    // ── min ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_min_empty_is_error() {
        let arr = make_array(vec![]);
        let result = unsafe { read_val(cel_k8s_list_min(arr)) };
        assert!(matches!(result, CelValue::Error(_)));
    }

    #[rstest]
    #[case::ints(vec![CelValue::Int(3), CelValue::Int(1), CelValue::Int(2)], CelValue::Int(1))]
    #[case::single(vec![CelValue::Int(42)], CelValue::Int(42))]
    fn test_min(#[case] elements: Vec<CelValue>, #[case] expected: CelValue) {
        let arr = make_array(elements);
        let result = unsafe { read_val(cel_k8s_list_min(arr)) };
        assert_eq!(result, expected);
    }

    // ── max ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_max_empty_is_error() {
        let arr = make_array(vec![]);
        let result = unsafe { read_val(cel_k8s_list_max(arr)) };
        assert!(matches!(result, CelValue::Error(_)));
    }

    #[rstest]
    #[case::ints(vec![CelValue::Int(1), CelValue::Int(3), CelValue::Int(2)], CelValue::Int(3))]
    #[case::single(vec![CelValue::Int(7)], CelValue::Int(7))]
    fn test_max(#[case] elements: Vec<CelValue>, #[case] expected: CelValue) {
        let arr = make_array(elements);
        let result = unsafe { read_val(cel_k8s_list_max(arr)) };
        assert_eq!(result, expected);
    }

    // ── indexOf ─────────────────────────────────────────────────────────────

    #[rstest]
    #[case::found(
        vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(2), CelValue::Int(3)],
        CelValue::Int(2),
        CelValue::Int(1)
    )]
    #[case::not_found(
        vec![CelValue::Int(1), CelValue::Int(2)],
        CelValue::Int(99),
        CelValue::Int(-1)
    )]
    #[case::empty(
        vec![],
        CelValue::String("x".to_string()),
        CelValue::Int(-1)
    )]
    fn test_index_of(
        #[case] elements: Vec<CelValue>,
        #[case] needle: CelValue,
        #[case] expected: CelValue,
    ) {
        let arr = make_array(elements);
        let needle_ptr = make_val(needle);
        let result = unsafe { read_val(cel_k8s_list_index_of(arr, needle_ptr)) };
        assert_eq!(result, expected);
    }

    // ── lastIndexOf ─────────────────────────────────────────────────────────

    #[rstest]
    #[case::found(
        vec![
            CelValue::String("a".to_string()),
            CelValue::String("b".to_string()),
            CelValue::String("b".to_string()),
            CelValue::String("c".to_string()),
        ],
        CelValue::String("b".to_string()),
        CelValue::Int(2)
    )]
    #[case::not_found(
        vec![CelValue::Double(1.0)],
        CelValue::Double(1.1),
        CelValue::Int(-1)
    )]
    fn test_last_index_of(
        #[case] elements: Vec<CelValue>,
        #[case] needle: CelValue,
        #[case] expected: CelValue,
    ) {
        let arr = make_array(elements);
        let needle_ptr = make_val(needle);
        let result = unsafe { read_val(cel_k8s_list_last_index_of(arr, needle_ptr)) };
        assert_eq!(result, expected);
    }
}
