//! Polymorphic iteration primitives for two-variable comprehensions.
//!
//! These functions abstract over list and map iteration so the compiler can emit
//! a single loop pattern that works for both range types:
//!
//! - **Lists**: `iter_var1` = `Int(index)`, `iter_var2` = `element`
//! - **Maps**: `iter_var1` = key, `iter_var2` = value
//!
//! Usage pattern emitted by the compiler:
//! ```text
//! prepared = cel_iter_prepare(range)
//! len      = cel_array_len(prepared)
//! loop i from 0..len:
//!   var1 = cel_iter_var1(range, prepared, i)
//!   var2 = cel_iter_var2(range, prepared, i)
//!   ... body using var1, var2 ...
//! ```

use crate::error::abort_with_error;
use crate::types::{CelMapKey, CelValue};
use slog::error;

/// Prepare a range value for two-variable comprehension iteration.
///
/// - For `CelValue::Array`: returns the array pointer itself (iteration index → element).
/// - For `CelValue::Object` (map): returns a new `CelValue::Array` of the map's keys.
///
/// The returned array is used with `cel_array_len` to drive the loop, and passed
/// alongside the original `range` to `cel_iter_var1`/`cel_iter_var2`.
///
/// # Safety
///
/// Caller must ensure `range_ptr` is a valid non-null pointer to a CelValue.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_iter_prepare(range_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if range_ptr.is_null() {
        error!(log, "range_ptr is null"; "function" => "cel_iter_prepare");
        abort_with_error("no such overload");
    }

    let range = unsafe { &*range_ptr };

    match range {
        CelValue::Array(_) => {
            // Lists: prepared = range itself. The compiler will use cel_array_len(prepared)
            // and cel_iter_var1/var2(range, prepared, i). For lists range == prepared.
            // We must NOT return range_ptr directly (would alias), so we return it — the
            // compiler is responsible for keeping range_ptr alive separately.
            // Actually we just return the same pointer; the compiler owns the local.
            range_ptr
        }
        CelValue::Object(hash_map) => {
            // Maps: prepared = array of keys
            let keys: Vec<CelValue> = hash_map
                .keys()
                .map(|k| match k {
                    CelMapKey::Bool(b) => CelValue::Bool(*b),
                    CelMapKey::Int(i) => CelValue::Int(*i),
                    CelMapKey::UInt(u) => CelValue::UInt(*u),
                    CelMapKey::String(s) => CelValue::String(s.clone()),
                })
                .collect();
            Box::into_raw(Box::new(CelValue::Array(keys)))
        }
        _ => {
            error!(log, "iter_prepare: expected list or map";
                "actual" => format!("{:?}", range));
            abort_with_error("no such overload")
        }
    }
}

/// Return the first iteration variable for the current step.
///
/// - For lists (`range` is Array): returns `CelValue::Int(index)` (a fresh allocation).
/// - For maps (`range` is Object): returns a clone of `prepared[index]` (the key).
///
/// `prepared` is the value returned by `cel_iter_prepare(range)`.
///
/// # Safety
///
/// Caller must ensure all pointers are valid non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_iter_var1(
    range_ptr: *mut CelValue,
    prepared_ptr: *mut CelValue,
    index: i32,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if range_ptr.is_null() || prepared_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_iter_var1");
        abort_with_error("no such overload");
    }

    let range = unsafe { &*range_ptr };

    match range {
        CelValue::Array(_) => {
            // For lists, var1 = Int(index)
            Box::into_raw(Box::new(CelValue::Int(index as i64)))
        }
        CelValue::Object(_) => {
            // For maps, var1 = key at position `index` in prepared keys array
            let prepared = unsafe { &*prepared_ptr };
            match prepared {
                CelValue::Array(keys) => {
                    let key = keys
                        .get(index as usize)
                        .unwrap_or_else(|| abort_with_error("index out of bounds"));
                    Box::into_raw(Box::new(key.clone()))
                }
                _ => abort_with_error("cel_iter_var1: prepared must be array"),
            }
        }
        _ => abort_with_error("cel_iter_var1: unsupported range type"),
    }
}

/// Return the second iteration variable for the current step.
///
/// - For lists (`range` is Array): returns a clone of `range[index]` (the element).
/// - For maps (`range` is Object): returns the value associated with `prepared[index]` (the key).
///
/// `prepared` is the value returned by `cel_iter_prepare(range)`.
///
/// # Safety
///
/// Caller must ensure all pointers are valid non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_iter_var2(
    range_ptr: *mut CelValue,
    prepared_ptr: *mut CelValue,
    index: i32,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if range_ptr.is_null() || prepared_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_iter_var2");
        abort_with_error("no such overload");
    }

    let range = unsafe { &*range_ptr };

    match range {
        CelValue::Array(arr) => {
            let elem = arr
                .get(index as usize)
                .unwrap_or_else(|| abort_with_error("index out of bounds"));
            Box::into_raw(Box::new(elem.clone()))
        }
        CelValue::Object(hash_map) => {
            // key = prepared[index]
            let prepared = unsafe { &*prepared_ptr };
            let key_val = match prepared {
                CelValue::Array(keys) => keys
                    .get(index as usize)
                    .unwrap_or_else(|| abort_with_error("index out of bounds")),
                _ => abort_with_error("cel_iter_var2: prepared must be array"),
            };
            let map_key = match CelMapKey::from_cel_value(key_val) {
                Some(k) => k,
                None => abort_with_error("cel_iter_var2: invalid map key type"),
            };
            match hash_map.get(&map_key) {
                Some(v) => Box::into_raw(Box::new(v.clone())),
                None => abort_with_error("cel_iter_var2: key not found in map"),
            }
        }
        _ => abort_with_error("cel_iter_var2: unsupported range type"),
    }
}

/// Conditional increment for `existsOne` comprehensions.
///
/// - `accu` must be `CelValue::Int`.
/// - `pred` must be `CelValue::Bool` or `CelValue::Error`.
///
/// Returns:
/// - `Int(accu + 1)` if `pred` is `Bool(true)`
/// - `accu` (cloned) if `pred` is `Bool(false)`
/// - the error `pred` if `pred` is `Error`
///
/// # Safety
///
/// Caller must ensure both pointers are valid non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_cond_inc(
    accu_ptr: *mut CelValue,
    pred_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if accu_ptr.is_null() || pred_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_cond_inc");
        abort_with_error("no such overload");
    }

    let accu = unsafe { &*accu_ptr };
    let pred = unsafe { &*pred_ptr };

    let count = match accu {
        CelValue::Int(n) => *n,
        _ => abort_with_error("cel_cond_inc: accu must be Int"),
    };

    match pred {
        CelValue::Bool(true) => Box::into_raw(Box::new(CelValue::Int(count + 1))),
        CelValue::Bool(false) => Box::into_raw(Box::new(CelValue::Int(count))),
        CelValue::Error(_) => Box::into_raw(Box::new(pred.clone())),
        _ => crate::error::create_error_value("no such overload"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{make_int, make_val, read_val};
    use rstest::rstest;

    // Helper: build a map CelValue pointer with given string→int entries
    fn make_map(entries: Vec<(&str, i64)>) -> *mut CelValue {
        let mut hm = std::collections::HashMap::new();
        for (k, v) in entries {
            hm.insert(CelMapKey::String(k.to_string()), CelValue::Int(v));
        }
        Box::into_raw(Box::new(CelValue::Object(hm)))
    }

    fn make_list(elements: Vec<CelValue>) -> *mut CelValue {
        Box::into_raw(Box::new(CelValue::Array(elements)))
    }

    // --- cel_iter_prepare ---

    #[test]
    fn test_iter_prepare_list_returns_same_pointer() {
        let list_ptr = make_list(vec![CelValue::Int(1), CelValue::Int(2)]);
        let prepared = unsafe { cel_iter_prepare(list_ptr) };
        // For lists, prepared IS range_ptr
        assert_eq!(prepared, list_ptr);
        unsafe {
            let _ = Box::from_raw(list_ptr);
        }
    }

    #[test]
    fn test_iter_prepare_map_returns_keys_array() {
        let map_ptr = make_map(vec![("a", 1)]);
        let prepared = unsafe { cel_iter_prepare(map_ptr) };
        let keys = read_val(prepared);
        match keys {
            CelValue::Array(arr) => {
                assert_eq!(arr.len(), 1);
                assert_eq!(arr[0], CelValue::String("a".to_string()));
            }
            _ => panic!("Expected Array of keys"),
        }
        unsafe {
            let _ = Box::from_raw(map_ptr);
        }
    }

    #[test]
    fn test_iter_prepare_empty_map() {
        let map_ptr = make_val(CelValue::Object(std::collections::HashMap::new()));
        let prepared = unsafe { cel_iter_prepare(map_ptr) };
        let keys = read_val(prepared);
        match keys {
            CelValue::Array(arr) => assert!(arr.is_empty()),
            _ => panic!("Expected empty Array"),
        }
        unsafe {
            let _ = Box::from_raw(map_ptr);
        }
    }

    // --- cel_iter_var1 for lists ---

    #[rstest]
    #[case(0, 0i64)]
    #[case(1, 1i64)]
    #[case(2, 2i64)]
    fn test_iter_var1_list_returns_index(#[case] idx: i32, #[case] expected: i64) {
        let list_ptr = make_list(vec![
            CelValue::Int(10),
            CelValue::Int(20),
            CelValue::Int(30),
        ]);
        let result_ptr = unsafe { cel_iter_var1(list_ptr, list_ptr, idx) };
        let result = read_val(result_ptr);
        assert_eq!(result, CelValue::Int(expected));
        unsafe {
            let _ = Box::from_raw(list_ptr);
        }
    }

    // --- cel_iter_var2 for lists ---

    #[rstest]
    #[case(0, CelValue::Int(10))]
    #[case(1, CelValue::Int(20))]
    #[case(2, CelValue::Int(30))]
    fn test_iter_var2_list_returns_element(#[case] idx: i32, #[case] expected: CelValue) {
        let list_ptr = make_list(vec![
            CelValue::Int(10),
            CelValue::Int(20),
            CelValue::Int(30),
        ]);
        let result_ptr = unsafe { cel_iter_var2(list_ptr, list_ptr, idx) };
        let result = read_val(result_ptr);
        assert_eq!(result, expected);
        unsafe {
            let _ = Box::from_raw(list_ptr);
        }
    }

    // --- cel_iter_var1 / var2 for maps ---

    #[test]
    fn test_iter_var1_map_returns_key() {
        let map_ptr = make_map(vec![("foo", 99)]);
        let prepared_ptr = unsafe { cel_iter_prepare(map_ptr) };
        let key_ptr = unsafe { cel_iter_var1(map_ptr, prepared_ptr, 0) };
        let key = read_val(key_ptr);
        assert_eq!(key, CelValue::String("foo".to_string()));
        unsafe {
            let _ = Box::from_raw(map_ptr);
            let _ = Box::from_raw(prepared_ptr);
        }
    }

    #[test]
    fn test_iter_var2_map_returns_value() {
        let map_ptr = make_map(vec![("bar", 42)]);
        let prepared_ptr = unsafe { cel_iter_prepare(map_ptr) };
        let val_ptr = unsafe { cel_iter_var2(map_ptr, prepared_ptr, 0) };
        let val = read_val(val_ptr);
        assert_eq!(val, CelValue::Int(42));
        unsafe {
            let _ = Box::from_raw(map_ptr);
            let _ = Box::from_raw(prepared_ptr);
        }
    }

    // --- cel_cond_inc ---

    #[test]
    fn test_cond_inc_true_increments() {
        let accu_ptr = make_int(2);
        let pred_ptr = make_val(CelValue::Bool(true));
        let result_ptr = unsafe { cel_cond_inc(accu_ptr, pred_ptr) };
        let result = read_val(result_ptr);
        assert_eq!(result, CelValue::Int(3));
        unsafe {
            let _ = Box::from_raw(accu_ptr);
            let _ = Box::from_raw(pred_ptr);
        }
    }

    #[test]
    fn test_cond_inc_false_keeps() {
        let accu_ptr = make_int(5);
        let pred_ptr = make_val(CelValue::Bool(false));
        let result_ptr = unsafe { cel_cond_inc(accu_ptr, pred_ptr) };
        let result = read_val(result_ptr);
        assert_eq!(result, CelValue::Int(5));
        unsafe {
            let _ = Box::from_raw(accu_ptr);
            let _ = Box::from_raw(pred_ptr);
        }
    }

    #[test]
    fn test_cond_inc_error_propagates() {
        let accu_ptr = make_int(0);
        let pred_ptr = make_val(CelValue::Error("oops".to_string()));
        let result_ptr = unsafe { cel_cond_inc(accu_ptr, pred_ptr) };
        let result = read_val(result_ptr);
        assert!(matches!(result, CelValue::Error(_)));
        unsafe {
            let _ = Box::from_raw(accu_ptr);
            let _ = Box::from_raw(pred_ptr);
        }
    }
}
