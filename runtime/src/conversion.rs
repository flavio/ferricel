//! Type conversion from CelValue to primitive types (i64, u64, bool).
//! These functions extract values from CelValue pointers and panic on type mismatches.
//! Also provides CEL type conversion functions (uint(), int(), double(), string(), timestamp(), duration()).

use crate::error::{null_to_unbound, abort_with_error};
use crate::helpers::cel_create_duration;
use crate::types::CelValue;
use slog::{debug, error};

// ---------------------------------------------------------------------------
// Non-consuming query: returns i64, borrows the pointer
// ---------------------------------------------------------------------------

/// Extract bool from a CelValue pointer, returned as i64 (0 or 1).
/// Does NOT consume the pointer (used by compiler control flow).
///
/// # Safety
/// `ptr` must be a valid, non-null pointer to an initialized CelValue.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_to_bool(ptr: *mut CelValue) -> i64 {
    let log = crate::logging::get_logger();

    if ptr.is_null() {
        error!(log, "Attempted to convert null CelValue pointer to bool";
            "function" => "cel_value_to_bool");
        abort_with_error("no such overload");
    }

    match unsafe { &*ptr } {
        CelValue::Bool(b) => {
            debug!(log, "Converting CelValue to bool"; "value" => *b);
            if *b { 1 } else { 0 }
        }
        other => {
            error!(log, "Type mismatch in conversion";
            "function" => "cel_value_to_bool",
            "expected" => "Bool",
            "actual" => format!("{:?}", other));
            abort_with_error("no such overload")
        }
    }
}

// ---------------------------------------------------------------------------
// Consuming CEL type-conversion functions
// ---------------------------------------------------------------------------

/// CEL uint() function. Consumes the operand.
///
/// # Safety
/// Pointer must be a valid, non-null, uniquely-owned CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { null_to_unbound(ptr) };
    Box::into_raw(Box::new(convert_uint(value)))
}

fn convert_uint(value: CelValue) -> CelValue {
    let log = crate::logging::get_logger();
    match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::UInt(u) => CelValue::UInt(u),
        CelValue::Int(i) => {
            if i < 0 {
                error!(log, "Cannot convert negative value to uint"; "value" => i);
                abort_with_error("no such overload");
            }
            CelValue::UInt(i as u64)
        }
        CelValue::Double(d) => {
            if d.is_nan() || d.is_infinite() {
                error!(log, "Cannot convert NaN or Infinity to uint"; "value" => format!("{}", d));
                abort_with_error("no such overload");
            }
            if d < 0.0 {
                error!(log, "Cannot convert negative value to uint"; "value" => d);
                abort_with_error("no such overload");
            }
            if d > u64::MAX as f64 {
                error!(log, "Value too large for uint"; "value" => d);
                abort_with_error("no such overload");
            }
            CelValue::UInt(d.trunc() as u64)
        }
        CelValue::String(s) => match s.parse::<u64>() {
            Ok(u) => CelValue::UInt(u),
            Err(_) => {
                error!(log, "Cannot parse string as uint"; "value" => s);
                abort_with_error("no such overload")
            }
        },
        other => {
            error!(log, "Cannot convert type to uint"; "from_type" => format!("{:?}", other));
            abort_with_error("no such overload")
        }
    }
}

/// CEL int() function. Consumes the operand.
///
/// # Safety
/// Pointer must be a valid, non-null, uniquely-owned CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_int(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { null_to_unbound(ptr) };
    Box::into_raw(Box::new(convert_int(value)))
}

fn convert_int(value: CelValue) -> CelValue {
    let log = crate::logging::get_logger();
    match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::Int(i) => CelValue::Int(i),
        CelValue::UInt(u) => {
            if u > i64::MAX as u64 {
                error!(log, "Value too large for int"; "value" => u);
                abort_with_error("no such overload");
            }
            CelValue::Int(u as i64)
        }
        CelValue::Double(d) => {
            if d.is_nan() || d.is_infinite() {
                error!(log, "Cannot convert NaN or Infinity to int"; "value" => format!("{}", d));
                abort_with_error("no such overload");
            }
            const MAX_SAFE_INT_AS_F64: f64 = 9223372036854774784.0;
            const MIN_SAFE_INT_AS_F64: f64 = -9223372036854774784.0;
            if d < MIN_SAFE_INT_AS_F64 || d > MAX_SAFE_INT_AS_F64 {
                error!(log, "Value out of range for int"; "value" => d);
                abort_with_error("no such overload");
            }
            CelValue::Int(d.trunc() as i64)
        }
        CelValue::String(s) => match s.parse::<i64>() {
            Ok(i) => CelValue::Int(i),
            Err(_) => {
                error!(log, "Cannot parse string as int"; "value" => s);
                abort_with_error("no such overload")
            }
        },
        CelValue::Timestamp(ts) => {
            debug!(log, "Converting Timestamp to int (Unix seconds)");
            CelValue::Int(ts.timestamp())
        }
        other => {
            error!(log, "Cannot convert type to int"; "from_type" => format!("{:?}", other));
            abort_with_error("no such overload")
        }
    }
}

/// CEL double() function. Consumes the operand.
///
/// # Safety
/// Pointer must be a valid, non-null, uniquely-owned CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_double(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { null_to_unbound(ptr) };
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
            Err(_) => {
                error!(log, "Cannot parse string as double"; "value" => s);
                abort_with_error("no such overload")
            }
        },
        other => {
            error!(log, "Cannot convert type to double"; "from_type" => format!("{:?}", other));
            abort_with_error("no such overload")
        }
    }
}

/// CEL timestamp() function. Consumes the operand.
///
/// # Safety
/// Pointer must be a valid, non-null, uniquely-owned CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { null_to_unbound(ptr) };
    Box::into_raw(Box::new(convert_timestamp(value)))
}

fn convert_timestamp(value: CelValue) -> CelValue {
    let log = crate::logging::get_logger();
    match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::Timestamp(ts) => CelValue::Timestamp(ts),
        CelValue::String(ref s) => {
            debug!(log, "Parsing string to timestamp"; "value" => s);
            let dt = crate::chrono_helpers::parse_rfc3339(s)
                .or_else(|_| {
                    let s_with_utc = format!("{}Z", s);
                    crate::chrono_helpers::parse_rfc3339(&s_with_utc)
                })
                .unwrap_or_else(|e| {
                    error!(log, "Cannot parse string as timestamp"; "value" => s, "error" => e);
                    abort_with_error("no such overload")
                });
            CelValue::Timestamp(dt)
        }
        CelValue::Int(seconds) => {
            debug!(log, "Converting int (Unix seconds) to timestamp"; "seconds" => seconds);
            use chrono::{TimeZone, Utc};
            let dt = Utc.timestamp_opt(seconds, 0).single().unwrap_or_else(|| {
                error!(log, "Invalid Unix timestamp"; "seconds" => seconds);
                abort_with_error("no such overload")
            });
            let dt_fixed = dt.with_timezone(&chrono::FixedOffset::east_opt(0).unwrap());
            CelValue::Timestamp(dt_fixed)
        }
        other => {
            error!(log, "Cannot convert type to timestamp"; "from_type" => format!("{:?}", other));
            abort_with_error("no such overload")
        }
    }
}

/// CEL duration() function. Consumes the operand.
///
/// # Safety
/// Pointer must be a valid, non-null, uniquely-owned CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_duration(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { null_to_unbound(ptr) };
    convert_duration_ptr(value)
}

fn convert_duration_ptr(value: CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();
    match value {
        CelValue::Error(e) => Box::into_raw(Box::new(CelValue::Error(e))),
        CelValue::Duration(d) => {
            debug!(log, "Duration identity conversion");
            let (seconds, nanos) = crate::chrono_helpers::duration_to_parts(&d);
            unsafe { cel_create_duration(seconds, nanos as i64) }
        }
        CelValue::String(s) => {
            debug!(log, "Parsing string to duration"; "value" => &s);
            let d = crate::chrono_helpers::parse_duration(&s).unwrap_or_else(|e| {
                error!(log, "Cannot parse string as duration"; "value" => &s, "error" => e);
                abort_with_error("no such overload")
            });
            let (seconds, nanos) = crate::chrono_helpers::duration_to_parts(&d);
            unsafe { cel_create_duration(seconds, nanos as i64) }
        }
        other => {
            error!(log, "Cannot convert type to duration"; "from_type" => format!("{:?}", other));
            abort_with_error("no such overload")
        }
    }
}

/// CEL bytes() function. Consumes the operand.
///
/// # Safety
/// Pointer must be a valid, non-null, uniquely-owned CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_bytes(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { null_to_unbound(ptr) };
    Box::into_raw(Box::new(convert_bytes(value)))
}

fn convert_bytes(value: CelValue) -> CelValue {
    let log = crate::logging::get_logger();
    match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::Bytes(b) => CelValue::Bytes(b),
        CelValue::String(s) => CelValue::Bytes(s.into_bytes()),
        other => {
            error!(log, "Cannot convert type to bytes"; "from_type" => format!("{:?}", other));
            abort_with_error("no such overload")
        }
    }
}

/// CEL bool() function. Consumes the operand.
///
/// # Safety
/// Pointer must be a valid, non-null, uniquely-owned CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_bool(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { null_to_unbound(ptr) };
    Box::into_raw(Box::new(convert_bool(value)))
}

fn convert_bool(value: CelValue) -> CelValue {
    let log = crate::logging::get_logger();
    match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::Bool(b) => CelValue::Bool(b),
        CelValue::String(s) => {
            let b = match s.as_str() {
                "true" | "TRUE" | "True" | "t" | "T" | "1" => true,
                "false" | "FALSE" | "False" | "f" | "F" | "0" => false,
                _ => {
                    error!(log, "Cannot parse string as bool"; "value" => s);
                    abort_with_error("no such overload")
                }
            };
            CelValue::Bool(b)
        }
        other => {
            error!(log, "Cannot convert type to bool"; "from_type" => format!("{:?}", other));
            abort_with_error("no such overload")
        }
    }
}

/// CEL string() function. Consumes the operand.
///
/// # Safety
/// Pointer must be a valid, non-null, uniquely-owned CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { null_to_unbound(ptr) };
    Box::into_raw(Box::new(convert_string(value)))
}

fn convert_string(value: CelValue) -> CelValue {
    let log = crate::logging::get_logger();
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
            Err(_) => {
                error!(log, "Invalid UTF-8 in bytes-to-string conversion");
                abort_with_error("no such overload")
            }
        },
        CelValue::IpAddr(addr) => CelValue::String(addr.to_string()),
        CelValue::Cidr(addr, prefix_len) => CelValue::String(format!("{}/{}", addr, prefix_len)),
        CelValue::Quantity(s) => CelValue::String(s),
        other => {
            error!(log, "Cannot convert type to string"; "from_type" => format!("{:?}", other));
            abort_with_error("no such overload")
        }
    }
}

/// CEL type() function. Consumes the operand.
///
/// # Safety
/// Pointer must be a valid, non-null, uniquely-owned CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_type(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { null_to_unbound(ptr) };
    let type_name = value.type_name().to_string();
    Box::into_raw(Box::new(CelValue::Type(type_name)))
}

#[cfg(test)]
mod tests {
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
}
