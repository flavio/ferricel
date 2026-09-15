//! Type conversion from CelValue to primitive types (i64, u64, bool).
//! Also provides CEL type conversion functions (uint(), int(), double(), string(), timestamp(), duration()).
//!
//! Every `convert_*` function here returns `CelValue::Error` for input it does not
//! accept, instead of aborting: this lets `||`, `&&`, and `?:` absorb the failure,
//! as the CEL spec requires. See the "Abort, or return an error value?" section of
//! `crate::error`. `cel_value_to_bool` is the one exception: the compiler only ever
//! calls it after ruling out `CelValue::Error`, so a non-`Bool` there is a compiler
//! bug, and aborting is correct.

use slog::debug;

use crate::{
    error::{CelError, abort_with_error, no_such_overload},
    memory::read_ptr,
    types::CelValue,
};

// ---------------------------------------------------------------------------
// Non-consuming query: returns i64, borrows the pointer
// ---------------------------------------------------------------------------

/// Extract bool from a CelValue pointer, returned as i64 (0 or 1).
/// Borrows the pointee in-place (used by compiler control flow).
///
/// The compiler only emits a call to this function after `IsError` has ruled
/// out `CelValue::Error` (see `operators.rs` and `collections.rs`), so a
/// non-`Bool` value here is a compiler bug, not a CEL runtime error. Abort is
/// correct.
///
/// # Safety
/// `ptr` must be a valid, non-null pointer to an initialized CelValue.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_to_bool(ptr: *mut CelValue) -> i64 {
    let log = crate::logging::get_logger();

    if ptr.is_null() {
        abort_with_error("cel_value_to_bool: null CelValue pointer, this is a compiler bug");
    }

    match unsafe { &*ptr } {
        CelValue::Bool(b) => {
            debug!(log, "Converting CelValue to bool"; "value" => *b);
            if *b { 1 } else { 0 }
        }
        other => abort_with_error(&format!(
            "cel_value_to_bool: expected Bool, got {other:?}; this is a compiler bug"
        )),
    }
}

// ---------------------------------------------------------------------------
// Consuming CEL type-conversion functions
// ---------------------------------------------------------------------------

/// CEL uint() function.
///
/// # Safety
/// Pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(ptr) };
    Box::into_raw(Box::new(convert_uint(value)))
}

fn convert_uint(value: CelValue) -> CelValue {
    let log = crate::logging::get_logger();
    match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::UInt(u) => CelValue::UInt(u),
        CelValue::Int(i) => {
            if i < 0 {
                debug!(log, "Cannot convert negative value to uint"; "value" => i);
                return CelValue::Error(CelError::new("range error converting int to uint"));
            }
            CelValue::UInt(i as u64)
        }
        CelValue::Double(d) => {
            if d.is_nan() || d.is_infinite() || d < 0.0 || d > u64::MAX as f64 {
                debug!(log, "Cannot convert double to uint"; "value" => d);
                return CelValue::Error(CelError::new("range error converting double to uint"));
            }
            CelValue::UInt(d.trunc() as u64)
        }
        CelValue::String(s) => match s.parse::<u64>() {
            Ok(u) => CelValue::UInt(u),
            Err(e) => {
                debug!(log, "Cannot parse string as uint"; "value" => &s, "error" => %e);
                CelValue::Error(CelError::new(format!(
                    "type conversion error from 'string' to 'uint': {e}"
                )))
            }
        },
        other => CelValue::Error(no_such_overload(&other)),
    }
}

/// CEL int() function.
///
/// # Safety
/// Pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_int(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(ptr) };
    Box::into_raw(Box::new(convert_int(value)))
}

fn convert_int(value: CelValue) -> CelValue {
    let log = crate::logging::get_logger();
    match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::Int(i) => CelValue::Int(i),
        CelValue::UInt(u) => {
            if u > i64::MAX as u64 {
                debug!(log, "Value too large for int"; "value" => u);
                return CelValue::Error(CelError::new("range error converting uint to int"));
            }
            CelValue::Int(u as i64)
        }
        CelValue::Double(d) => {
            if d.is_nan() || d.is_infinite() {
                debug!(log, "Cannot convert NaN or Infinity to int"; "value" => format!("{d}"));
                return CelValue::Error(CelError::new("range error converting double to int"));
            }
            const MAX_SAFE_INT_AS_F64: f64 = 9223372036854774784.0;
            const MIN_SAFE_INT_AS_F64: f64 = -9223372036854774784.0;
            if !(MIN_SAFE_INT_AS_F64..=MAX_SAFE_INT_AS_F64).contains(&d) {
                debug!(log, "Value out of range for int"; "value" => d);
                return CelValue::Error(CelError::new("range error converting double to int"));
            }
            CelValue::Int(d.trunc() as i64)
        }
        CelValue::String(s) => match s.parse::<i64>() {
            Ok(i) => CelValue::Int(i),
            Err(e) => {
                debug!(log, "Cannot parse string as int"; "value" => &s, "error" => %e);
                CelValue::Error(CelError::new(format!(
                    "type conversion error from 'string' to 'int': {e}"
                )))
            }
        },
        CelValue::Timestamp(ts) => {
            debug!(log, "Converting Timestamp to int (Unix seconds)");
            CelValue::Int(ts.timestamp())
        }
        other => CelValue::Error(no_such_overload(&other)),
    }
}

/// CEL double() function.
///
/// # Safety
/// Pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_double(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(ptr) };
    Box::into_raw(Box::new(convert_double(value)))
}

fn convert_double(value: CelValue) -> CelValue {
    let log = crate::logging::get_logger();
    match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::Double(d) => CelValue::Double(d),
        CelValue::Int(i) => CelValue::Double(i as f64),
        CelValue::UInt(u) => CelValue::Double(u as f64),
        CelValue::String(s) => match s.parse::<f64>() {
            Ok(d) => CelValue::Double(d),
            Err(e) => {
                debug!(log, "Cannot parse string as double"; "value" => &s, "error" => %e);
                CelValue::Error(CelError::new(format!(
                    "type conversion error from 'string' to 'double': {e}"
                )))
            }
        },
        other => CelValue::Error(no_such_overload(&other)),
    }
}

/// CEL timestamp() function.
///
/// # Safety
/// Pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(ptr) };
    Box::into_raw(Box::new(convert_timestamp(value)))
}

fn convert_timestamp(value: CelValue) -> CelValue {
    let log = crate::logging::get_logger();
    match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::Timestamp(ts) => CelValue::Timestamp(ts),
        CelValue::String(ref s) => {
            debug!(log, "Parsing string to timestamp"; "value" => s);
            let parsed = crate::chrono_helpers::parse_rfc3339(s).or_else(|_| {
                let s_with_utc = format!("{s}Z");
                crate::chrono_helpers::parse_rfc3339(&s_with_utc)
            });
            match parsed {
                Ok(dt) => CelValue::Timestamp(dt),
                Err(e) => {
                    debug!(log, "Cannot parse string as timestamp"; "value" => s, "error" => %e);
                    CelValue::Error(CelError::new(format!(
                        "invalid timestamp string {s:?}: {e}"
                    )))
                }
            }
        }
        CelValue::Int(seconds) => {
            debug!(log, "Converting int (Unix seconds) to timestamp"; "seconds" => seconds);
            use chrono::{TimeZone, Utc};
            match Utc.timestamp_opt(seconds, 0).single() {
                Some(dt) => {
                    let dt_fixed = dt.with_timezone(&chrono::FixedOffset::east_opt(0).unwrap());
                    CelValue::Timestamp(dt_fixed)
                }
                None => {
                    debug!(log, "Invalid Unix timestamp"; "seconds" => seconds);
                    CelValue::Error(CelError::new("timestamp overflow"))
                }
            }
        }
        other => CelValue::Error(no_such_overload(&other)),
    }
}

/// CEL duration() function.
///
/// # Safety
/// Pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_duration(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(ptr) };
    Box::into_raw(Box::new(convert_duration(value)))
}

fn convert_duration(value: CelValue) -> CelValue {
    let log = crate::logging::get_logger();
    match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::Duration(d) => {
            debug!(log, "Duration identity conversion");
            CelValue::Duration(d)
        }
        CelValue::String(s) => {
            debug!(log, "Parsing string to duration"; "value" => &s);
            match crate::chrono_helpers::parse_duration(&s) {
                Ok(d) => CelValue::Duration(d),
                Err(e) => {
                    debug!(log, "Cannot parse string as duration"; "value" => &s, "error" => %e);
                    CelValue::Error(CelError::new(format!("invalid duration string {s:?}: {e}")))
                }
            }
        }
        other => CelValue::Error(no_such_overload(&other)),
    }
}

/// CEL bytes() function.
///
/// # Safety
/// Pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_bytes(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(ptr) };
    Box::into_raw(Box::new(convert_bytes(value)))
}

fn convert_bytes(value: CelValue) -> CelValue {
    match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::Bytes(b) => CelValue::Bytes(b),
        CelValue::String(s) => CelValue::Bytes(s.into_bytes()),
        other => CelValue::Error(no_such_overload(&other)),
    }
}

/// CEL bool() function.
///
/// # Safety
/// Pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_bool(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(ptr) };
    Box::into_raw(Box::new(convert_bool(value)))
}

fn convert_bool(value: CelValue) -> CelValue {
    let log = crate::logging::get_logger();
    match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::Bool(b) => CelValue::Bool(b),
        CelValue::String(s) => match s.as_str() {
            "true" | "TRUE" | "True" | "t" | "T" | "1" => CelValue::Bool(true),
            "false" | "FALSE" | "False" | "f" | "F" | "0" => CelValue::Bool(false),
            _ => {
                debug!(log, "Cannot parse string as bool"; "value" => &s);
                CelValue::Error(CelError::new(format!(
                    "type conversion error from 'string' to 'bool': {s:?}"
                )))
            }
        },
        other => CelValue::Error(no_such_overload(&other)),
    }
}

/// CEL string() function.
///
/// # Safety
/// Pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(ptr) };
    Box::into_raw(Box::new(convert_string(value)))
}

fn convert_string(value: CelValue) -> CelValue {
    match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::String(s) => CelValue::String(s),
        CelValue::Int(i) => CelValue::String(i.to_string()),
        CelValue::UInt(u) => CelValue::String(u.to_string()),
        CelValue::Double(d) => CelValue::String(d.to_string()),
        CelValue::Bool(b) => CelValue::String(if b { "true".into() } else { "false".into() }),
        CelValue::Timestamp(dt) => {
            let s = if dt.offset().local_minus_utc() == 0 {
                dt.format("%Y-%m-%dT%H:%M:%S%.fZ").to_string()
            } else {
                dt.to_rfc3339()
            };
            CelValue::String(s)
        }
        CelValue::Duration(d) => CelValue::String(crate::chrono_helpers::format_duration(&d)),
        CelValue::Bytes(bytes) => match std::str::from_utf8(&bytes) {
            Ok(s) => CelValue::String(s.to_string()),
            Err(_) => CelValue::Error(CelError::new("invalid UTF-8 in bytes-to-string conversion")),
        },
        CelValue::IpAddr(addr) => CelValue::String(addr.to_string()),
        CelValue::Cidr(addr, prefix_len) => CelValue::String(format!("{addr}/{prefix_len}")),
        CelValue::Quantity(s) => CelValue::String(s),
        CelValue::Semver(v) => CelValue::String(v.to_string()),
        other => CelValue::Error(no_such_overload(&other)),
    }
}

/// CEL type() function.
///
/// # Safety
/// Pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_type(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(ptr) };
    let type_name = value.type_name().to_string();
    Box::into_raw(Box::new(CelValue::Type(type_name)))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn test_value_to_bool_true() {
        let ptr = Box::into_raw(Box::new(CelValue::Bool(true)));
        unsafe {
            let result = cel_value_to_bool(ptr);
            assert_eq!(result, 1);
            // cel_value_to_bool does NOT consume — free manually
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_value_to_bool_false() {
        let ptr = Box::into_raw(Box::new(CelValue::Bool(false)));
        unsafe {
            let result = cel_value_to_bool(ptr);
            assert_eq!(result, 0);
            let _ = Box::from_raw(ptr);
        }
    }

    /// Every `convert_*` propagates an incoming `CelValue::Error` unchanged,
    /// instead of masking it with a generic message. This is what lets an
    /// error value (a missing key, an unbound variable) survive through a
    /// conversion and still reach `||` for absorption.
    #[rstest]
    #[case::uint(convert_uint as fn(CelValue) -> CelValue)]
    #[case::int(convert_int as fn(CelValue) -> CelValue)]
    #[case::double(convert_double as fn(CelValue) -> CelValue)]
    #[case::timestamp(convert_timestamp as fn(CelValue) -> CelValue)]
    #[case::duration(convert_duration as fn(CelValue) -> CelValue)]
    #[case::bytes(convert_bytes as fn(CelValue) -> CelValue)]
    #[case::bool_(convert_bool as fn(CelValue) -> CelValue)]
    #[case::string(convert_string as fn(CelValue) -> CelValue)]
    fn convert_propagates_input_error(#[case] convert: fn(CelValue) -> CelValue) {
        let original = CelError::new("no such key: 'x'");
        let result = convert(CelValue::Error(original.clone()));
        assert_eq!(result, CelValue::Error(original));
    }

    #[rstest]
    #[case::negative(CelValue::Int(-1), "range error")]
    #[case::nan(CelValue::Double(f64::NAN), "range error")]
    #[case::bad_string(CelValue::String("x".into()), "type conversion error")]
    #[case::wrong_type(CelValue::Bool(true), "no such overload")]
    fn uint_rejects_bad_input(#[case] input: CelValue, #[case] expected_substring: &str) {
        let CelValue::Error(err) = convert_uint(input) else {
            panic!("expected an error value");
        };
        assert!(
            err.message.contains(expected_substring),
            "expected {expected_substring:?} in {:?}",
            err.message
        );
    }

    #[rstest]
    #[case::too_large_uint(CelValue::UInt(u64::MAX), "range error")]
    #[case::nan(CelValue::Double(f64::NAN), "range error")]
    #[case::out_of_range_double(CelValue::Double(1e30), "range error")]
    #[case::bad_string(CelValue::String("x".into()), "type conversion error")]
    #[case::wrong_type(CelValue::Bool(true), "no such overload")]
    fn int_rejects_bad_input(#[case] input: CelValue, #[case] expected_substring: &str) {
        let CelValue::Error(err) = convert_int(input) else {
            panic!("expected an error value");
        };
        assert!(
            err.message.contains(expected_substring),
            "expected {expected_substring:?} in {:?}",
            err.message
        );
    }

    #[rstest]
    #[case::bad_string(CelValue::String("not a number".into()), "type conversion error")]
    #[case::wrong_type(CelValue::Bool(true), "no such overload")]
    fn double_rejects_bad_input(#[case] input: CelValue, #[case] expected_substring: &str) {
        let CelValue::Error(err) = convert_double(input) else {
            panic!("expected an error value");
        };
        assert!(err.message.contains(expected_substring));
    }

    #[rstest]
    #[case::bad_string(CelValue::String("not a timestamp".into()))]
    #[case::wrong_type(CelValue::Bool(true))]
    fn timestamp_rejects_bad_input(#[case] input: CelValue) {
        assert!(matches!(convert_timestamp(input), CelValue::Error(_)));
    }

    #[test]
    fn timestamp_int_out_of_range_is_an_error() {
        let CelValue::Error(err) = convert_timestamp(CelValue::Int(i64::MAX)) else {
            panic!("expected an error value");
        };
        assert!(err.message.contains("overflow"));
    }

    #[rstest]
    #[case::bad_string(CelValue::String("not a duration".into()))]
    #[case::wrong_type(CelValue::Bool(true))]
    fn duration_rejects_bad_input(#[case] input: CelValue) {
        assert!(matches!(convert_duration(input), CelValue::Error(_)));
    }

    #[test]
    fn bytes_rejects_unsupported_type() {
        assert!(matches!(
            convert_bytes(CelValue::Bool(true)),
            CelValue::Error(_)
        ));
    }

    #[rstest]
    #[case::bad_string(CelValue::String("maybe".into()))]
    #[case::wrong_type(CelValue::Int(1))]
    fn bool_rejects_bad_input(#[case] input: CelValue) {
        assert!(matches!(convert_bool(input), CelValue::Error(_)));
    }

    #[test]
    fn string_rejects_invalid_utf8_bytes() {
        let bad = CelValue::Bytes(vec![0xff, 0xfe]);
        let CelValue::Error(err) = convert_string(bad) else {
            panic!("expected an error value");
        };
        assert!(err.message.contains("UTF-8"));
    }
}
