//! CEL optional type runtime functions.
//!
//! Implements the CEL optional functions:
//!   - `optional.none()`              → `optional<dyn>`  (no value)
//!   - `optional.of(x)`               → `optional<dyn>`  (always wraps x)
//!   - `optional.ofNonZeroValue(x)`   → `optional<dyn>`  (wraps x unless x is zero/empty/null)
//!   - `<opt>.hasValue()`             → bool
//!   - `<opt>.value()`                → dyn  (or error if none)
//!   - `<opt>.orValue(default)`       → dyn  (unwrap or return default)
//!   - `<opt>.or(other_opt)`          → `optional<dyn>`  (first with value, or other_opt)
//!
//! `optMap` and `optFlatMap` are macro-like constructs that the compiler
//! handles by inlining the lambda; they do not need dedicated runtime functions.
//!
//! Reference: CEL spec optional types extension.

use slog::error;

use crate::{
    error::{create_error_value, read_ptr},
    types::{CelMapKey, CelValue},
};

// ─────────────────────────────────────────────────────────────────────────────
// Zero-value test
// ─────────────────────────────────────────────────────────────────────────────

/// Returns true if `val` is considered a "zero value" for `ofNonZeroValue`.
///
/// Per CEL spec:
/// - null → zero
/// - false → zero
/// - 0 (int/uint/double) → zero
/// - "" (string) → zero
/// - b"" (bytes) → zero
/// - [] (list) → zero
/// - {} (map) → zero
/// - Everything else (including Optional(None), Optional(Some(...))) → non-zero
///
/// For proto struct objects (Object maps containing `__type__` metadata),
/// the struct is zero if it has no user-visible fields (only internal metadata).
pub(crate) fn is_zero_value(val: &CelValue) -> bool {
    match val {
        CelValue::Null => true,
        CelValue::Bool(b) => !b,
        CelValue::Int(n) => *n == 0,
        CelValue::UInt(n) => *n == 0,
        CelValue::Double(f) => *f == 0.0,
        CelValue::String(s) => s.is_empty(),
        CelValue::Bytes(b) => b.is_empty(),
        CelValue::Array(a) => a.is_empty(),
        // Timestamp is zero if it represents the Unix epoch (1970-01-01T00:00:00Z),
        // matching cel-go's Timestamp.IsZeroValue() which returns t.IsZero().
        CelValue::Timestamp(dt) => dt.timestamp() == 0 && dt.timestamp_subsec_nanos() == 0,
        // Duration is zero if it spans no time, matching cel-go's Duration.IsZeroValue().
        CelValue::Duration(d) => d.is_zero(),
        CelValue::Object(m) => {
            // A plain map is zero if empty.
            // A proto struct (has "__type__" key) is zero if it has no user-visible fields —
            // i.e., all keys are internal metadata (start with "__" and end with "__").
            let has_type_key =
                m.contains_key(&crate::types::CelMapKey::String("__type__".to_string()));
            if has_type_key {
                // Count only non-metadata keys
                let user_field_count = m
                    .keys()
                    .filter(|k| {
                        let s = k.to_string_key();
                        !(s.starts_with("__") && s.ends_with("__"))
                    })
                    .count();
                user_field_count == 0
            } else {
                m.is_empty()
            }
        }
        _ => false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Wasm-callable runtime functions
// ─────────────────────────────────────────────────────────────────────────────

/// `optional.none()` → Optional(None)
///
/// Returns an optional with no value.
#[unsafe(no_mangle)]
pub extern "C" fn cel_optional_none() -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::Optional(None)))
}

/// `optional.of(x)` → Optional(Some(x))
///
/// Wraps any value in an optional, including null and zero values.
/// Reads `val_ptr`.
///
/// # Safety
/// `val_ptr` must be a valid, non-null pointer to a `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_optional_of(val_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();
    if val_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_optional_of");
        return create_error_value("no such overload");
    }
    let val = unsafe { read_ptr(val_ptr) };
    Box::into_raw(Box::new(CelValue::Optional(Some(Box::new(val)))))
}

/// `optional.ofNonZeroValue(x)` → Optional(Some(x)) if x is non-zero, else Optional(None)
///
/// Reads `val_ptr`.
///
/// # Safety
/// `val_ptr` must be a valid, non-null pointer to a `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_optional_of_non_zero_value(val_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();
    if val_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_optional_of_non_zero_value");
        return create_error_value("no such overload");
    }
    let val = unsafe { read_ptr(val_ptr) };
    if is_zero_value(&val) {
        Box::into_raw(Box::new(CelValue::Optional(None)))
    } else {
        Box::into_raw(Box::new(CelValue::Optional(Some(Box::new(val)))))
    }
}

/// `<opt>.hasValue()` → bool
///
/// Returns true if the optional contains a value.
///
/// # Safety
/// `opt_ptr` must be a valid, non-null pointer to a `CelValue::Optional`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_optional_has_value(opt_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();
    if opt_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_optional_has_value");
        return create_error_value("no such overload");
    }
    let val = unsafe { &*opt_ptr };
    match val {
        CelValue::Optional(inner) => Box::into_raw(Box::new(CelValue::Bool(inner.is_some()))),
        // For non-optional values treated as present (e.g. after field access on concrete value)
        other => {
            error!(log, "expected Optional"; "function" => "cel_optional_has_value", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

/// `<opt>.value()` → inner value or Error
///
/// Unwraps the optional. Returns an error if empty. Reads `opt_ptr`.
///
/// # Safety
/// `opt_ptr` must be a valid, non-null pointer to a `CelValue::Optional`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_optional_value(opt_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();
    if opt_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_optional_value");
        return create_error_value("no such overload");
    }
    let val = unsafe { read_ptr(opt_ptr) };
    match val {
        CelValue::Optional(Some(inner)) => Box::into_raw(inner),
        CelValue::Optional(None) => {
            error!(log, "optional is empty"; "function" => "cel_optional_value");
            create_error_value("optional.none() dereference")
        }
        other => {
            error!(log, "expected Optional"; "function" => "cel_optional_value", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

/// `<opt>.orValue(default)` → inner value if present, else default
///
/// Reads both `opt_ptr` and `default_ptr`.
///
/// # Safety
/// `opt_ptr` must be a valid, non-null pointer to a `CelValue::Optional`.
/// `default_ptr` must be a valid, non-null pointer to a `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_optional_or_value(
    opt_ptr: *mut CelValue,
    default_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();
    if opt_ptr.is_null() || default_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_optional_or_value");
        return create_error_value("no such overload");
    }
    let opt = unsafe { read_ptr(opt_ptr) };
    let default = unsafe { read_ptr(default_ptr) };
    match opt {
        CelValue::Optional(Some(inner)) => Box::into_raw(inner),
        CelValue::Optional(None) => Box::into_raw(Box::new(default)),
        other => {
            error!(log, "expected Optional"; "function" => "cel_optional_or_value", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

/// `<opt>.or(other_opt)` → first optional with a value, or the second
///
/// If `opt` has a value, returns `opt`. Otherwise returns `other_opt`.
/// Reads both `opt_ptr` and `other_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_optional_or(
    opt_ptr: *mut CelValue,
    other_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();
    if opt_ptr.is_null() || other_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_optional_or");
        return create_error_value("no such overload");
    }
    let opt = unsafe { read_ptr(opt_ptr) };
    let other = unsafe { read_ptr(other_ptr) };
    Box::into_raw(Box::new(match opt {
        CelValue::Optional(Some(_)) => opt,
        CelValue::Optional(None) => other,
        other_val => {
            error!(log, "expected Optional"; "function" => "cel_optional_or", "got" => format!("{:?}", other_val));
            return create_error_value("no such overload");
        }
    }))
}

/// `receiver?.field` — CEL optional select operator (`_?._`)
///
/// Semantics:
/// - `Optional(None)?.field` → `Optional(None)`
/// - `Optional(Some(Object(map)))?.field` → `Optional(Some(map[field]))` or `Optional(None)` if absent
/// - `Optional(Some(other))?.field` → error ("no such key") — can't access field on non-map
/// - `Object(map)?.field` → `Optional(Some(map[field]))` or `Optional(None)` if absent
/// - anything else → error ("no such key")
///
/// Reads `receiver_ptr`. The `i32` params are raw Wasm memory addresses.
///
/// # Safety
/// `receiver_ptr` must be a valid non-null pointer to a `CelValue`.
/// `field_name_ptr` must point to valid UTF-8 bytes of length `field_name_len`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_optional_select(
    receiver_ptr: *mut CelValue,
    field_name_ptr: i32,
    field_name_len: i32,
) -> *mut CelValue {
    use std::slice;
    let log = crate::logging::get_logger();

    if receiver_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_optional_select");
        return create_error_value("no such key");
    }

    let field_name = unsafe {
        let bytes = slice::from_raw_parts(field_name_ptr as *const u8, field_name_len as usize);
        match String::from_utf8(bytes.to_vec()) {
            Ok(s) => s,
            Err(_) => return create_error_value("no such key"),
        }
    };

    let receiver = unsafe { read_ptr(receiver_ptr) };

    fn select_from_map(
        map: &std::collections::HashMap<CelMapKey, CelValue>,
        field_name: &str,
    ) -> *mut CelValue {
        let key = CelMapKey::String(field_name.to_string());
        match map.get(&key) {
            Some(v) => Box::into_raw(Box::new(CelValue::Optional(Some(Box::new(v.clone()))))),
            None => Box::into_raw(Box::new(CelValue::Optional(None))),
        }
    }

    match receiver {
        CelValue::Optional(None) => Box::into_raw(Box::new(CelValue::Optional(None))),
        CelValue::Optional(Some(inner)) => match inner.as_ref() {
            CelValue::Object(map) => select_from_map(map, &field_name),
            // Accessing a field on a non-map/object value wrapped in Optional is an error
            _ => create_error_value("no such key"),
        },
        CelValue::Object(ref map) => select_from_map(map, &field_name),
        // Accessing a field on a non-map/object value is an error
        _ => create_error_value("no such key"),
    }
}

/// `receiver[?key]` — CEL optional index operator (`_[?_]`)
///
/// Semantics:
/// - `Optional(None)[?key]` → `Optional(None)`
/// - `Optional(Some(container))[?key]` → recursively index container
/// - `Array[?i]` → `Optional(Some(array[i]))` or `Optional(None)` if out of bounds
/// - `Object(map)[?key]` → `Optional(Some(map[key]))` or `Optional(None)` if absent
///
/// Reads both `container_ptr` and `key_ptr`.
///
/// # Safety
/// Both pointers must be valid and non-null.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_optional_index(
    container_ptr: *mut CelValue,
    key_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if container_ptr.is_null() || key_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_optional_index");
        return Box::into_raw(Box::new(CelValue::Optional(None)));
    }

    let container = unsafe { read_ptr(container_ptr) };
    let key = unsafe { read_ptr(key_ptr) };

    fn index_array(arr: &[CelValue], key: &CelValue) -> *mut CelValue {
        let idx = match key {
            CelValue::Int(i) => {
                if *i < 0 {
                    return Box::into_raw(Box::new(CelValue::Optional(None)));
                }
                *i as usize
            }
            CelValue::UInt(u) => *u as usize,
            _ => return Box::into_raw(Box::new(CelValue::Optional(None))),
        };
        match arr.get(idx) {
            Some(v) => Box::into_raw(Box::new(CelValue::Optional(Some(Box::new(v.clone()))))),
            None => Box::into_raw(Box::new(CelValue::Optional(None))),
        }
    }

    fn index_map(
        map: &std::collections::HashMap<CelMapKey, CelValue>,
        key: &CelValue,
    ) -> *mut CelValue {
        // Try exact key match first
        let cel_key = match CelMapKey::from_cel_value(key) {
            Some(k) => k,
            None => {
                // Double keys: try coercing to int/uint if the value is a whole number
                if let CelValue::Double(f) = key {
                    let f = *f;
                    if f.fract() == 0.0 && f.is_finite() {
                        if f >= 0.0 && f <= u64::MAX as f64 {
                            let as_uint = CelMapKey::UInt(f as u64);
                            if let Some(v) = map.get(&as_uint) {
                                return Box::into_raw(Box::new(CelValue::Optional(Some(
                                    Box::new(v.clone()),
                                ))));
                            }
                        }
                        if f >= i64::MIN as f64 && f <= i64::MAX as f64 {
                            let as_int = CelMapKey::Int(f as i64);
                            if let Some(v) = map.get(&as_int) {
                                return Box::into_raw(Box::new(CelValue::Optional(Some(
                                    Box::new(v.clone()),
                                ))));
                            }
                        }
                    }
                }
                return Box::into_raw(Box::new(CelValue::Optional(None)));
            }
        };
        if let Some(v) = map.get(&cel_key) {
            return Box::into_raw(Box::new(CelValue::Optional(Some(Box::new(v.clone())))));
        }
        // Try numeric cross-type equality: int/uint interop
        // Per CEL spec, integer keys are cross-comparable across int/uint
        let alt_key = match &cel_key {
            CelMapKey::Int(i) => {
                if *i >= 0 {
                    Some(CelMapKey::UInt(*i as u64))
                } else {
                    None
                }
            }
            CelMapKey::UInt(u) => {
                if *u <= i64::MAX as u64 {
                    Some(CelMapKey::Int(*u as i64))
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(alt) = alt_key
            && let Some(v) = map.get(&alt)
        {
            return Box::into_raw(Box::new(CelValue::Optional(Some(Box::new(v.clone())))));
        }
        Box::into_raw(Box::new(CelValue::Optional(None)))
    }

    match container {
        CelValue::Optional(None) => Box::into_raw(Box::new(CelValue::Optional(None))),
        CelValue::Optional(Some(ref inner)) => match inner.as_ref() {
            CelValue::Array(arr) => index_array(arr, &key),
            CelValue::Object(map) => index_map(map, &key),
            _ => Box::into_raw(Box::new(CelValue::Optional(None))),
        },
        CelValue::Array(ref arr) => index_array(arr, &key),
        CelValue::Object(ref map) => index_map(map, &key),
        _ => Box::into_raw(Box::new(CelValue::Optional(None))),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn make_val(v: CelValue) -> *mut CelValue {
        Box::into_raw(Box::new(v))
    }

    unsafe fn read_val(ptr: *mut CelValue) -> CelValue {
        let v = unsafe { (*ptr).clone() };
        unsafe { drop(Box::from_raw(ptr)) };
        v
    }

    fn some(v: CelValue) -> CelValue {
        CelValue::Optional(Some(Box::new(v)))
    }

    fn none() -> CelValue {
        CelValue::Optional(None)
    }

    // ── is_zero_value ────────────────────────────────────────────────────────

    #[rstest]
    #[case::null(CelValue::Null, true)]
    #[case::bool_false(CelValue::Bool(false), true)]
    #[case::bool_true(CelValue::Bool(true), false)]
    #[case::int_zero(CelValue::Int(0), true)]
    #[case::int_nonzero(CelValue::Int(42), false)]
    #[case::uint_zero(CelValue::UInt(0), true)]
    #[case::uint_nonzero(CelValue::UInt(1), false)]
    #[case::double_zero(CelValue::Double(0.0), true)]
    #[case::double_nonzero(CelValue::Double(1.5), false)]
    #[case::string_empty(CelValue::String("".to_string()), true)]
    #[case::string_nonempty(CelValue::String("x".to_string()), false)]
    #[case::bytes_empty(CelValue::Bytes(vec![]), true)]
    #[case::bytes_nonempty(CelValue::Bytes(vec![1]), false)]
    #[case::array_empty(CelValue::Array(vec![]), true)]
    #[case::array_nonempty(CelValue::Array(vec![CelValue::Int(1)]), false)]
    #[case::timestamp_epoch(
        CelValue::Timestamp(crate::chrono_helpers::parts_to_datetime(0, 0)),
        true
    )]
    #[case::timestamp_nonzero(
        CelValue::Timestamp(crate::chrono_helpers::parts_to_datetime(1, 0)),
        false
    )]
    #[case::duration_zero(CelValue::Duration(chrono::Duration::zero()), true)]
    #[case::duration_nonzero(CelValue::Duration(chrono::Duration::seconds(1)), false)]
    #[case::optional_none(none(), false)]
    #[case::optional_some(some(CelValue::Int(0)), false)]
    fn test_is_zero_value(#[case] val: CelValue, #[case] expected: bool) {
        assert_eq!(is_zero_value(&val), expected);
    }

    // ── optional.none() ──────────────────────────────────────────────────────

    #[test]
    fn test_none() {
        let val = unsafe { read_val(cel_optional_none()) };
        assert_eq!(val, none());
    }

    // ── optional.of() ────────────────────────────────────────────────────────

    #[rstest]
    #[case::int(CelValue::Int(42), some(CelValue::Int(42)))]
    #[case::null(CelValue::Null, some(CelValue::Null))]
    fn test_of(#[case] input: CelValue, #[case] expected: CelValue) {
        let val = unsafe { read_val(cel_optional_of(make_val(input))) };
        assert_eq!(val, expected);
    }

    // ── optional.ofNonZeroValue() ────────────────────────────────────────────

    #[rstest]
    #[case::nonzero_int(CelValue::Int(42), some(CelValue::Int(42)))]
    #[case::zero_int(CelValue::Int(0), none())]
    #[case::null(CelValue::Null, none())]
    #[case::empty_string(CelValue::String("".to_string()), none())]
    #[case::nonempty_string(CelValue::String("hi".to_string()), some(CelValue::String("hi".to_string())))]
    fn test_of_non_zero_value(#[case] input: CelValue, #[case] expected: CelValue) {
        let val = unsafe { read_val(cel_optional_of_non_zero_value(make_val(input))) };
        assert_eq!(val, expected);
    }

    // ── <opt>.hasValue() ─────────────────────────────────────────────────────

    #[rstest]
    #[case::some(some(CelValue::Int(1)), CelValue::Bool(true))]
    #[case::none(none(), CelValue::Bool(false))]
    fn test_has_value(#[case] opt: CelValue, #[case] expected: CelValue) {
        let opt_ptr = make_val(opt);
        let result = unsafe { read_val(cel_optional_has_value(opt_ptr)) };
        // has_value borrows only — free the input
        unsafe { drop(Box::from_raw(opt_ptr)) };
        assert_eq!(result, expected);
    }

    // ── <opt>.value() ────────────────────────────────────────────────────────

    #[test]
    fn test_value_some() {
        let val = unsafe { read_val(cel_optional_value(make_val(some(CelValue::Int(99))))) };
        assert_eq!(val, CelValue::Int(99));
    }

    #[test]
    fn test_value_none_returns_error() {
        let val = unsafe { read_val(cel_optional_value(make_val(none()))) };
        assert!(matches!(val, CelValue::Error(_)));
    }

    // ── <opt>.orValue() ──────────────────────────────────────────────────────

    #[rstest]
    #[case::some_returns_inner(some(CelValue::Int(5)), CelValue::Int(99), CelValue::Int(5))]
    #[case::none_returns_default(none(), CelValue::Int(99), CelValue::Int(99))]
    fn test_or_value(#[case] opt: CelValue, #[case] default: CelValue, #[case] expected: CelValue) {
        let val = unsafe { read_val(cel_optional_or_value(make_val(opt), make_val(default))) };
        assert_eq!(val, expected);
    }

    // ── <opt>.or() ───────────────────────────────────────────────────────────

    #[rstest]
    #[case::some_wins(some(CelValue::Int(1)), some(CelValue::Int(2)), some(CelValue::Int(1)))]
    #[case::none_falls_through(none(), some(CelValue::Int(42)), some(CelValue::Int(42)))]
    fn test_or(#[case] opt: CelValue, #[case] other: CelValue, #[case] expected: CelValue) {
        let val = unsafe { read_val(cel_optional_or(make_val(opt), make_val(other))) };
        assert_eq!(val, expected);
    }
}
