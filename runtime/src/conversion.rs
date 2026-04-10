//! Type conversion from CelValue to primitive types (i64, u64, bool).
//! These functions extract values from CelValue pointers and panic on type mismatches.
//! Also provides CEL type conversion functions (uint(), int(), double(), string(), timestamp(), duration()).

use crate::error::abort_with_error;
use crate::helpers::{cel_create_double, cel_create_duration, cel_create_int, cel_create_uint};
use crate::types::CelValue;
use slog::{debug, error};

/// Extract i64 from a CelValue pointer.
///
/// # Parameters
/// - `ptr`: Pointer to a CelValue (must be Int variant)
///
/// # Returns
/// - The i64 value
///
/// # Panics
/// - If ptr is null
/// - If CelValue is not Int variant
///
/// # Safety
/// - `ptr` must be a valid pointer to a CelValue
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_to_i64(ptr: *mut CelValue) -> i64 {
    let log = crate::logging::get_logger();

    if ptr.is_null() {
        error!(log, "Attempted to convert null CelValue pointer to i64";
            "function" => "cel_value_to_i64");
        abort_with_error("no such overload");
    }

    // SAFETY: Caller guarantees ptr is valid
    let value = unsafe { &*ptr };

    match value {
        CelValue::Int(n) => {
            debug!(log, "Converting CelValue to i64"; "value" => *n);
            *n
        }
        other => {
            error!(log, "Type mismatch in conversion";
            "function" => "cel_value_to_i64",
            "expected" => "Int",
            "actual" => format!("{:?}", other));
            abort_with_error("no such overload")
        }
    }
}

/// Extract u64 from a CelValue pointer.
///
/// # Parameters
/// - `ptr`: Pointer to a CelValue (must be UInt variant)
///
/// # Returns
/// - The u64 value
///
/// # Panics
/// - If ptr is null
/// - If CelValue is not UInt variant
///
/// # Safety
/// - `ptr` must be a valid pointer to a CelValue
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_to_u64(ptr: *mut CelValue) -> u64 {
    let log = crate::logging::get_logger();

    if ptr.is_null() {
        error!(log, "Attempted to convert null CelValue pointer to u64";
            "function" => "cel_value_to_u64");
        abort_with_error("no such overload");
    }

    // SAFETY: Caller guarantees ptr is valid
    let value = unsafe { &*ptr };

    match value {
        CelValue::UInt(n) => {
            debug!(log, "Converting CelValue to u64"; "value" => *n);
            *n
        }
        other => {
            error!(log, "Type mismatch in conversion";
            "function" => "cel_value_to_u64",
            "expected" => "UInt",
            "actual" => format!("{:?}", other));
            abort_with_error("no such overload")
        }
    }
}

/// Extract bool from a CelValue pointer, returned as i64 (0 or 1).
///
/// # Parameters
/// - `ptr`: Pointer to a CelValue (must be Bool variant)
///
/// # Returns
/// - 1 if true, 0 if false
///
/// # Panics
/// - If ptr is null
/// - If CelValue is not Bool variant
///
/// # Safety
/// - `ptr` must be a valid pointer to a CelValue
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_to_bool(ptr: *mut CelValue) -> i64 {
    let log = crate::logging::get_logger();

    if ptr.is_null() {
        error!(log, "Attempted to convert null CelValue pointer to bool";
            "function" => "cel_value_to_bool");
        abort_with_error("no such overload");
    }

    // SAFETY: Caller guarantees ptr is valid
    let value = unsafe { &*ptr };

    match value {
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

// CEL Type Conversion Functions
// These implement the CEL spec type conversion functions: uint(), int(), double(), string()

/// CEL uint() function - converts values to uint.
/// Signatures per CEL spec:
/// - uint(uint) -> uint (identity)
/// - uint(int) -> uint (type conversion, panics on negative)
/// - uint(double) -> uint (rounds toward zero, panics if out of range)
/// - uint(string) -> uint (parses decimal string, panics on error)
///
/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer. The caller must ensure:
/// - `ptr` is a valid, properly aligned pointer to an initialized CelValue
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_uint(ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if ptr.is_null() {
            error!(log, "Cannot convert null to uint";
                "function" => "cel_uint");
            abort_with_error("no such overload");
        }

        match &*ptr {
            CelValue::UInt(u) => cel_create_uint(*u),
            CelValue::Int(i) => {
                if *i < 0 {
                    error!(log, "Cannot convert negative value to uint";
                        "function" => "cel_uint",
                        "from_type" => "Int",
                        "value" => *i);
                    abort_with_error("no such overload");
                }
                cel_create_uint(*i as u64)
            }
            CelValue::Double(d) => {
                if d.is_nan() || d.is_infinite() {
                    {
                        error!(log, "Cannot convert NaN or Infinity to uint";
                        "function" => "cel_uint",
                        "from_type" => "Double",
                        "value" => format!("{}", d));
                        abort_with_error("no such overload")
                    }
                }
                if *d < 0.0 {
                    error!(log, "Cannot convert negative value to uint";
                        "function" => "cel_uint",
                        "from_type" => "Double",
                        "value" => *d);
                    abort_with_error("no such overload");
                }
                if *d > u64::MAX as f64 {
                    error!(log, "Value too large for uint";
                        "function" => "cel_uint",
                        "from_type" => "Double",
                        "value" => *d,
                        "max" => u64::MAX);
                    abort_with_error("no such overload");
                }
                cel_create_uint(d.trunc() as u64)
            }
            CelValue::String(s) => match s.parse::<u64>() {
                Ok(u) => cel_create_uint(u),
                Err(_) => {
                    error!(log, "Cannot parse string as uint";
                    "function" => "cel_uint",
                    "value" => s);
                    abort_with_error("no such overload")
                }
            },
            other => {
                error!(log, "Cannot convert type to uint";
                "function" => "cel_uint",
                "from_type" => format!("{:?}", other));
                abort_with_error("no such overload")
            }
        }
    }
}

/// CEL int() function - converts values to int.
/// Signatures per CEL spec:
/// - int(int) -> int (identity)
/// - int(uint) -> int (type conversion, panics on overflow)
/// - int(double) -> int (rounds toward zero, panics if out of range)
/// - int(string) -> int (parses decimal string, panics on error)
///
/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer. The caller must ensure:
/// - `ptr` is a valid, properly aligned pointer to an initialized CelValue
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_int(ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if ptr.is_null() {
            error!(log, "Cannot convert null to int";
                "function" => "cel_int");
            abort_with_error("no such overload");
        }

        match &*ptr {
            CelValue::Int(i) => cel_create_int(*i),
            CelValue::UInt(u) => {
                if *u > i64::MAX as u64 {
                    error!(log, "Value too large for int";
                        "function" => "cel_int",
                        "from_type" => "UInt",
                        "value" => *u,
                        "max" => i64::MAX);
                    abort_with_error("no such overload");
                }
                cel_create_int(*u as i64)
            }
            CelValue::Double(d) => {
                if d.is_nan() || d.is_infinite() {
                    {
                        error!(log, "Cannot convert NaN or Infinity to int";
                        "function" => "cel_int",
                        "from_type" => "Double",
                        "value" => format!("{}", d));
                        abort_with_error("no such overload")
                    }
                }
                // i64::MIN and i64::MAX cannot be exactly represented as f64
                // Values at or beyond these boundaries should error
                // Safe range is approximately: -9223372036854774784.0 to 9223372036854774784.0
                const MAX_SAFE_INT_AS_F64: f64 = 9223372036854774784.0;
                const MIN_SAFE_INT_AS_F64: f64 = -9223372036854774784.0;

                if *d < MIN_SAFE_INT_AS_F64 || *d > MAX_SAFE_INT_AS_F64 {
                    error!(log, "Value out of range for int";
                        "function" => "cel_int",
                        "from_type" => "Double",
                        "value" => *d,
                        "min" => i64::MIN,
                        "max" => i64::MAX);
                    abort_with_error("no such overload");
                }
                cel_create_int(d.trunc() as i64)
            }
            CelValue::String(s) => match s.parse::<i64>() {
                Ok(i) => cel_create_int(i),
                Err(_) => {
                    error!(log, "Cannot parse string as int";
                    "function" => "cel_int",
                    "value" => s);
                    abort_with_error("no such overload")
                }
            },
            CelValue::Timestamp(ts) => {
                // Convert timestamp to Unix seconds (int)
                debug!(log, "Converting Timestamp to int (Unix seconds)"; "timestamp" => format!("{:?}", ts));
                cel_create_int(ts.timestamp())
            }
            other => {
                error!(log, "Cannot convert type to int";
                "function" => "cel_int",
                "from_type" => format!("{:?}", other));
                abort_with_error("no such overload")
            }
        }
    }
}

/// CEL double() function - converts values to double.
/// Signatures per CEL spec:
/// - double(double) -> double (identity)
/// - double(int) -> double (type conversion)
/// - double(uint) -> double (type conversion)
/// - double(string) -> double (parses string, panics on error)
///
/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer. The caller must ensure:
/// - `ptr` is a valid, properly aligned pointer to an initialized CelValue
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_double(ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if ptr.is_null() {
            error!(log, "Cannot convert null to double";
                "function" => "cel_double");
            abort_with_error("no such overload");
        }

        match &*ptr {
            CelValue::Double(d) => cel_create_double(*d),
            CelValue::Int(i) => cel_create_double(*i as f64),
            CelValue::UInt(u) => cel_create_double(*u as f64),
            CelValue::String(s) => match s.parse::<f64>() {
                Ok(d) => cel_create_double(d),
                Err(_) => {
                    error!(log, "Cannot parse string as double";
                    "function" => "cel_double",
                    "value" => s);
                    abort_with_error("no such overload")
                }
            },
            other => {
                error!(log, "Cannot convert type to double";
                "function" => "cel_double",
                "from_type" => format!("{:?}", other));
                abort_with_error("no such overload")
            }
        }
    }
}

/// CEL timestamp() function - converts values to timestamp.
/// Signatures per CEL spec:
/// - timestamp(timestamp) -> timestamp (identity)
/// - timestamp(string) -> timestamp (parses RFC3339 format)
///
/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer. The caller must ensure:
/// - `ptr` is a valid, properly aligned pointer to an initialized CelValue
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp(ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if ptr.is_null() {
            error!(log, "Cannot convert null to timestamp";
                "function" => "cel_timestamp");
            abort_with_error("no such overload");
        }

        match &*ptr {
            CelValue::Timestamp(_) => {
                // Already a timestamp - return as-is (identity conversion)
                debug!(log, "Timestamp identity conversion");
                ptr
            }
            CelValue::String(s) => {
                debug!(log, "Parsing string to timestamp"; "value" => s);
                // Parse RFC3339 - preserves timezone from string, assumes UTC if missing
                let dt = crate::chrono_helpers::parse_rfc3339(s)
                    .or_else(|_| {
                        // If parse fails, try appending 'Z' for UTC assumption
                        // This handles strings like "2024-01-15T10:30:00" without timezone
                        let s_with_utc = format!("{}Z", s);
                        crate::chrono_helpers::parse_rfc3339(&s_with_utc)
                    })
                    .unwrap_or_else(|e| {
                        error!(log, "Cannot parse string as timestamp";
                            "function" => "cel_timestamp",
                            "value" => s,
                            "error" => e);
                        abort_with_error("no such overload")
                    });

                // Create CelValue::Timestamp directly - preserves timezone!
                Box::into_raw(Box::new(CelValue::Timestamp(dt)))
            }
            CelValue::Int(seconds) => {
                // Convert Unix seconds (int) to timestamp
                debug!(log, "Converting int (Unix seconds) to timestamp"; "seconds" => *seconds);
                use chrono::{TimeZone, Utc};
                let dt = Utc.timestamp_opt(*seconds, 0).single().unwrap_or_else(|| {
                    error!(log, "Invalid Unix timestamp";
                            "function" => "cel_timestamp",
                            "seconds" => *seconds);
                    abort_with_error("no such overload")
                });
                // Convert UTC to FixedOffset (UTC has offset +00:00)
                let dt_fixed = dt.with_timezone(&chrono::FixedOffset::east_opt(0).unwrap());
                Box::into_raw(Box::new(CelValue::Timestamp(dt_fixed)))
            }
            other => {
                error!(log, "Cannot convert type to timestamp";
                "function" => "cel_timestamp",
                "from_type" => format!("{:?}", other));
                abort_with_error("no such overload")
            }
        }
    }
}

/// CEL duration() function - converts values to duration.
/// Signatures per CEL spec:
/// - duration(duration) -> duration (identity)
/// - duration(string) -> duration (parses CEL duration format like "1h30m")
///
/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer. The caller must ensure:
/// - `ptr` is a valid, properly aligned pointer to an initialized CelValue
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_duration(ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if ptr.is_null() {
            error!(log, "Cannot convert null to duration";
                "function" => "cel_duration");
            abort_with_error("no such overload");
        }

        match &*ptr {
            CelValue::Duration(d) => {
                debug!(log, "Duration identity conversion");
                let (seconds, nanos) = crate::chrono_helpers::duration_to_parts(d);
                cel_create_duration(seconds, nanos as i64)
            }
            CelValue::String(s) => {
                debug!(log, "Parsing string to duration"; "value" => s);
                let d = crate::chrono_helpers::parse_duration(s).unwrap_or_else(|e| {
                    error!(log, "Cannot parse string as duration";
                            "function" => "cel_duration",
                            "value" => s,
                            "error" => e);
                    abort_with_error("no such overload")
                });
                let (seconds, nanos) = crate::chrono_helpers::duration_to_parts(&d);
                cel_create_duration(seconds, nanos as i64)
            }
            other => {
                error!(log, "Cannot convert type to duration";
                "function" => "cel_duration",
                "from_type" => format!("{:?}", other));
                abort_with_error("no such overload")
            }
        }
    }
}

/// CEL bytes() function - converts values to bytes.
/// Signatures per CEL spec:
/// - bytes(bytes) -> bytes (identity)
/// - bytes(string) -> bytes (UTF-8 encode string to bytes)
///
/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer. The caller must ensure:
/// - `ptr` is a valid, properly aligned pointer to an initialized CelValue
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_bytes(ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if ptr.is_null() {
            error!(log, "Cannot convert null to bytes";
                "function" => "cel_bytes");
            abort_with_error("no such overload");
        }

        match &*ptr {
            CelValue::Bytes(b) => {
                debug!(log, "Bytes identity conversion");
                Box::into_raw(Box::new(CelValue::Bytes(b.clone())))
            }
            CelValue::String(s) => {
                debug!(log, "Converting String to bytes"; "length" => s.len());
                // Convert string to UTF-8 bytes
                Box::into_raw(Box::new(CelValue::Bytes(s.as_bytes().to_vec())))
            }
            other => {
                error!(log, "Cannot convert type to bytes";
                "function" => "cel_bytes",
                "from_type" => format!("{:?}", other));
                abort_with_error("no such overload")
            }
        }
    }
}

/// CEL bool() function - converts values to bool.
/// Signatures per CEL spec:
/// - bool(bool) -> bool (identity)
/// - bool(string) -> bool (parses "true"/"false" with various cases: "1", "t", "T", "TRUE", "True", "0", "f", "F", "FALSE", "False")
///
/// # Safety
///
/// `ptr` must be a valid pointer to a `CelValue` allocated by this runtime, or null.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_bool(ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if ptr.is_null() {
            error!(log, "Cannot convert null to bool";
                "function" => "cel_bool");
            abort_with_error("no such overload");
        }

        match &*ptr {
            CelValue::Bool(b) => {
                debug!(log, "Bool identity conversion");
                Box::into_raw(Box::new(CelValue::Bool(*b)))
            }
            CelValue::String(s) => {
                debug!(log, "Converting String to bool"; "value" => s);
                // Per CEL spec, bool() accepts various string representations
                let b = match s.as_str() {
                    "true" | "TRUE" | "True" | "t" | "T" | "1" => true,
                    "false" | "FALSE" | "False" | "f" | "F" | "0" => false,
                    _ => {
                        error!(log, "Cannot parse string as bool";
                        "function" => "cel_bool",
                        "value" => s);
                        abort_with_error("no such overload")
                    }
                };
                Box::into_raw(Box::new(CelValue::Bool(b)))
            }
            other => {
                error!(log, "Cannot convert type to bool";
                "function" => "cel_bool",
                "from_type" => format!("{:?}", other));
                abort_with_error("no such overload")
            }
        }
    }
}

/// CEL string() function - converts values to string.
/// Handles all CEL types including timestamp, duration, and bytes formatting.
/// For bytes, validates UTF-8 and panics on invalid sequences per CEL spec.
///
/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer. The caller must ensure:
/// - `ptr` is a valid, properly aligned pointer to an initialized CelValue
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string(ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if ptr.is_null() {
            error!(log, "Cannot convert null to string";
                "function" => "cel_string");
            abort_with_error("no such overload");
        }

        match &*ptr {
            CelValue::String(s) => {
                debug!(log, "String identity conversion");
                Box::into_raw(Box::new(CelValue::String(s.clone())))
            }
            CelValue::Int(i) => {
                debug!(log, "Converting Int to string"; "value" => *i);
                Box::into_raw(Box::new(CelValue::String(i.to_string())))
            }
            CelValue::UInt(u) => {
                debug!(log, "Converting UInt to string"; "value" => *u);
                Box::into_raw(Box::new(CelValue::String(u.to_string())))
            }
            CelValue::Double(d) => {
                debug!(log, "Converting Double to string"; "value" => *d);
                Box::into_raw(Box::new(CelValue::String(d.to_string())))
            }
            CelValue::Bool(b) => {
                debug!(log, "Converting Bool to string"; "value" => *b);
                Box::into_raw(Box::new(CelValue::String(if *b {
                    "true".to_string()
                } else {
                    "false".to_string()
                })))
            }
            CelValue::Timestamp(dt) => {
                debug!(log, "Converting Timestamp to string");
                // Use "Z" suffix for UTC timestamps instead of "+00:00" for CEL compliance
                let s = if dt.offset().local_minus_utc() == 0 {
                    dt.format("%Y-%m-%dT%H:%M:%S%.fZ").to_string()
                } else {
                    dt.to_rfc3339()
                };
                Box::into_raw(Box::new(CelValue::String(s)))
            }
            CelValue::Duration(d) => {
                debug!(log, "Converting Duration to string");
                let s = crate::chrono_helpers::format_duration(d);
                Box::into_raw(Box::new(CelValue::String(s)))
            }
            CelValue::Bytes(bytes) => {
                debug!(log, "Converting Bytes to string");
                // Convert bytes to UTF-8 string, error on invalid UTF-8 per CEL spec
                match std::str::from_utf8(bytes) {
                    Ok(s) => Box::into_raw(Box::new(CelValue::String(s.to_string()))),
                    Err(_) => {
                        error!(log, "Invalid UTF-8 in bytes-to-string conversion";
                        "function" => "cel_string",
                        "from_type" => "Bytes");
                        abort_with_error("no such overload")
                    }
                }
            }
            CelValue::IpAddr(addr) => {
                debug!(log, "Converting IpAddr to string");
                Box::into_raw(Box::new(CelValue::String(addr.to_string())))
            }
            CelValue::Cidr(addr, prefix_len) => {
                debug!(log, "Converting Cidr to string");
                Box::into_raw(Box::new(CelValue::String(format!(
                    "{}/{}",
                    addr, prefix_len
                ))))
            }
            other => {
                error!(log, "Cannot convert type to string";
                "function" => "cel_string",
                "from_type" => format!("{:?}", other));
                abort_with_error("no such overload")
            }
        }
    }
}

/// CEL type() function - returns the type of a value as a Type value.
/// Signatures per CEL spec:
/// - type(value) -> Type (returns runtime type)
///
/// # Safety
///
/// `ptr` must be a valid pointer to a `CelValue` allocated by this runtime, or null.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_type(ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if ptr.is_null() {
            error!(log, "Cannot get type of null";
                "function" => "cel_type");
            abort_with_error("no such overload");
        }

        let type_name = match &*ptr {
            CelValue::Null => "null_type",
            CelValue::Bool(_) => "bool",
            CelValue::Int(_) => "int",
            CelValue::UInt(_) => "uint",
            CelValue::Double(_) => "double",
            CelValue::String(_) => "string",
            CelValue::Bytes(_) => "bytes",
            CelValue::Array(_) => "list",
            CelValue::Object(_) => "map",
            CelValue::Timestamp(_) => "google.protobuf.Timestamp",
            CelValue::Duration(_) => "google.protobuf.Duration",
            CelValue::Type(_) => "type",
            CelValue::Error(_) => "error",
            CelValue::Url(_, _) => "url",
            CelValue::IpAddr(_) => "net.IP",
            CelValue::Cidr(_, _) => "net.CIDR",
            CelValue::Semver(_) => "semver",
        };

        debug!(log, "Getting type of value"; "type_name" => type_name);
        Box::into_raw(Box::new(CelValue::Type(type_name.to_string())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_to_i64_positive() {
        let value = Box::new(CelValue::Int(42));
        let ptr = Box::into_raw(value);

        unsafe {
            let result = cel_value_to_i64(ptr);
            assert_eq!(result, 42);

            // Cleanup
            let _boxed = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_value_to_i64_negative() {
        let value = Box::new(CelValue::Int(-100));
        let ptr = Box::into_raw(value);

        unsafe {
            let result = cel_value_to_i64(ptr);
            assert_eq!(result, -100);

            // Cleanup
            let _boxed = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_value_to_i64_zero() {
        let value = Box::new(CelValue::Int(0));
        let ptr = Box::into_raw(value);

        unsafe {
            let result = cel_value_to_i64(ptr);
            assert_eq!(result, 0);

            // Cleanup
            let _boxed = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_value_to_bool_true() {
        let value = Box::new(CelValue::Bool(true));
        let ptr = Box::into_raw(value);

        unsafe {
            let result = cel_value_to_bool(ptr);
            assert_eq!(result, 1);

            // Cleanup
            let _boxed = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_value_to_bool_false() {
        let value = Box::new(CelValue::Bool(false));
        let ptr = Box::into_raw(value);

        unsafe {
            let result = cel_value_to_bool(ptr);
            assert_eq!(result, 0);

            // Cleanup
            let _boxed = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_value_to_u64_basic() {
        let value = Box::new(CelValue::UInt(12345));
        let ptr = Box::into_raw(value);

        unsafe {
            let result = cel_value_to_u64(ptr);
            assert_eq!(result, 12345);

            // Cleanup
            let _boxed = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_value_to_u64_max() {
        let value = Box::new(CelValue::UInt(u64::MAX));
        let ptr = Box::into_raw(value);

        unsafe {
            let result = cel_value_to_u64(ptr);
            assert_eq!(result, u64::MAX);

            // Cleanup
            let _boxed = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_value_to_u64_zero() {
        let value = Box::new(CelValue::UInt(0));
        let ptr = Box::into_raw(value);

        unsafe {
            let result = cel_value_to_u64(ptr);
            assert_eq!(result, 0);

            // Cleanup
            let _boxed = Box::from_raw(ptr);
        }
    }

    // Note: Panic tests removed because they cause issues with custom allocator in test environment
    // The panic behavior is tested indirectly through integration tests
}
