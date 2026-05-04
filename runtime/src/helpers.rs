//! Helper functions for creating and extracting CelValue pointers.
//! These are used internally by other runtime functions and exported for compiler use.
//! Also includes polymorphic operators that dispatch to type-specific implementations.

use slog::{debug, error};

use crate::{
    arithmetic, array, bytes,
    error::{abort_with_error, read_ptr},
    string, temporal,
    types::CelValue,
};

/// Creates a CelValue::Int on the heap and returns a pointer to it.
///
/// # Safety
///
/// This function is unsafe because it returns a raw pointer. The caller must ensure:
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_create_int(value: i64) -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::Int(value)))
}

/// Creates a CelValue::UInt on the heap and returns a pointer to it.
///
/// # Safety
///
/// This function is unsafe because it returns a raw pointer. The caller must ensure:
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_create_uint(value: u64) -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::UInt(value)))
}

/// Creates a CelValue::Bool on the heap and returns a pointer to it.
/// Input: i64 where 0 = false, non-zero = true
///
/// # Safety
///
/// This function is unsafe because it returns a raw pointer. The caller must ensure:
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_create_bool(value: i64) -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::Bool(value != 0)))
}

/// Creates a CelValue::Double on the heap and returns a pointer to it.
///
/// # Safety
///
/// This function is unsafe because it returns a raw pointer. The caller must ensure:
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_create_double(value: f64) -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::Double(value)))
}

/// Creates a CelValue::Timestamp on the heap and returns a pointer to it.
///
/// # Arguments
/// * `seconds` - Seconds since Unix epoch (1970-01-01T00:00:00Z)
/// * `nanos` - Nanoseconds component (0-999,999,999)
///
/// # Safety
///
/// This function is unsafe because it returns a raw pointer. The caller must ensure:
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_create_timestamp(seconds: i64, nanos: i64) -> *mut CelValue {
    let dt = crate::chrono_helpers::parts_to_datetime(seconds, nanos);
    Box::into_raw(Box::new(CelValue::Timestamp(dt)))
}

/// Creates a CelValue::Duration on the heap and returns a pointer to it.
///
/// # Arguments
/// * `seconds` - Number of seconds (can be negative)
/// * `nanos` - Nanoseconds component (0-999,999,999 or negative)
///
/// # Safety
///
/// This function is unsafe because it returns a raw pointer. The caller must ensure:
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_create_duration(seconds: i64, nanos: i64) -> *mut CelValue {
    let duration = crate::chrono_helpers::parts_to_duration(seconds, nanos);
    Box::into_raw(Box::new(CelValue::Duration(duration)))
}

/// Creates a CelValue::Null on the heap and returns a pointer to it.
///
/// # Safety
///
/// This function is unsafe because it returns a raw pointer. The caller must ensure:
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_create_null() -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::Null))
}

/// Creates a CelValue::Type on the heap from a string pointer and returns a pointer to it.
///
/// # Arguments
/// * `type_name_ptr` - Pointer to the type name string in Wasm memory
/// * `type_name_len` - Length of the type name string
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - `type_name_ptr` points to valid UTF-8 bytes in Wasm memory
/// - `type_name_len` is the correct length of the type name
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_create_type(
    type_name_ptr: *const u8,
    type_name_len: i32,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if type_name_ptr.is_null() {
            error!(log, "Null pointer for type name";
                "function" => "cel_create_type");
            abort_with_error("no such overload");
        }

        let slice = std::slice::from_raw_parts(type_name_ptr, type_name_len as usize);
        let type_name = String::from_utf8_lossy(slice).to_string();

        debug!(log, "Creating Type value"; "type_name" => &type_name);
        Box::into_raw(Box::new(CelValue::Type(type_name)))
    }
}

/// Creates a CelValue::Error on the heap from a string pointer and returns a pointer to it.
///
/// # Arguments
/// * `error_msg_ptr` - Pointer to the error message string in Wasm memory
/// * `error_msg_len` - Length of the error message string
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - `error_msg_ptr` points to valid bytes in Wasm memory (if not null)
/// - `error_msg_len` is the correct length of the error message
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_create_error(
    error_msg_ptr: *const u8,
    error_msg_len: i32,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if error_msg_ptr.is_null() {
            error!(log, "Null pointer for error message";
                "function" => "cel_create_error");
            // Even when creating an error fails, we still need to return an error
            return Box::into_raw(Box::new(CelValue::Error("unknown error".to_string())));
        }

        let slice = std::slice::from_raw_parts(error_msg_ptr, error_msg_len as usize);
        let error_msg = String::from_utf8_lossy(slice).to_string();

        debug!(log, "Creating Error value"; "error_msg" => &error_msg);
        Box::into_raw(Box::new(CelValue::Error(error_msg)))
    }
}

/// Test-only helper: Extracts i64 from a CelValue::Int pointer, or aborts.
#[cfg(test)]
pub(crate) fn extract_int(ptr: *mut CelValue) -> i64 {
    unsafe {
        match &*ptr {
            CelValue::Int(i) => *i,
            other => crate::error::abort_with_error(&format!("expected Int, got {:?}", other)),
        }
    }
}

/// Polymorphic addition operator.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_add(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = unsafe { read_ptr(a_ptr) };
    let b = unsafe { read_ptr(b_ptr) };
    let log = crate::logging::get_logger();
    let result = match (a, b) {
        (CelValue::Error(e), _) => CelValue::Error(e),
        (_, CelValue::Error(e)) => CelValue::Error(e),
        (CelValue::Int(a), CelValue::Int(b)) => {
            debug!(log, "Performing Int addition"; "left" => a, "right" => b);
            match a.checked_add(b) {
                Some(r) => CelValue::Int(r),
                None => {
                    error!(log, "Integer overflow in addition"; "left" => a, "right" => b);
                    CelValue::Error("return error for overflow".into())
                }
            }
        }
        (CelValue::UInt(a), CelValue::UInt(b)) => {
            debug!(log, "Performing UInt addition"; "left" => a, "right" => b);
            match a.checked_add(b) {
                Some(r) => CelValue::UInt(r),
                None => {
                    error!(log, "UInt overflow in addition"; "left" => a, "right" => b);
                    CelValue::Error("return error for overflow".into())
                }
            }
        }
        (CelValue::Double(a), CelValue::Double(b)) => {
            CelValue::Double(arithmetic::double_add(a, b))
        }
        (CelValue::String(a), CelValue::String(b)) => {
            CelValue::String(string::cel_string_concat(&a, &b))
        }
        (CelValue::Bytes(a), CelValue::Bytes(b)) => {
            CelValue::Bytes(bytes::cel_bytes_concat_internal(&a, &b))
        }
        (CelValue::Array(a), CelValue::Array(b)) => {
            CelValue::Array(array::cel_array_concat(&a, &b))
        }
        (CelValue::Timestamp(ts), CelValue::Duration(dur)) => {
            temporal::timestamp_add_duration_inner(ts, dur)
        }
        (CelValue::Duration(dur), CelValue::Timestamp(ts)) => {
            temporal::timestamp_add_duration_inner(ts, dur)
        }
        (CelValue::Duration(d1), CelValue::Duration(d2)) => temporal::duration_add_inner(d1, d2),
        (a, b) => {
            error!(log, "Cannot add incompatible types";
                "left_type" => format!("{:?}", a),
                "right_type" => format!("{:?}", b));
            CelValue::Error("no such overload".into())
        }
    };
    Box::into_raw(Box::new(result))
}

/// Polymorphic subtraction operator.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_sub(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = unsafe { read_ptr(a_ptr) };
    let b = unsafe { read_ptr(b_ptr) };
    let log = crate::logging::get_logger();
    let result = match (a, b) {
        (CelValue::Error(e), _) => CelValue::Error(e),
        (_, CelValue::Error(e)) => CelValue::Error(e),
        (CelValue::Int(a), CelValue::Int(b)) => {
            debug!(log, "Performing Int subtraction"; "left" => a, "right" => b);
            match a.checked_sub(b) {
                Some(r) => CelValue::Int(r),
                None => {
                    error!(log, "Integer overflow in subtraction"; "left" => a, "right" => b);
                    CelValue::Error("return error for overflow".into())
                }
            }
        }
        (CelValue::UInt(a), CelValue::UInt(b)) => {
            debug!(log, "Performing UInt subtraction"; "left" => a, "right" => b);
            match a.checked_sub(b) {
                Some(r) => CelValue::UInt(r),
                None => {
                    error!(log, "UInt underflow in subtraction"; "left" => a, "right" => b);
                    CelValue::Error("return error for overflow".into())
                }
            }
        }
        (CelValue::Double(a), CelValue::Double(b)) => {
            CelValue::Double(arithmetic::double_sub(a, b))
        }
        (CelValue::Timestamp(ts), CelValue::Duration(dur)) => {
            temporal::timestamp_sub_duration_inner(ts, dur)
        }
        (CelValue::Timestamp(ts1), CelValue::Timestamp(ts2)) => {
            temporal::timestamp_diff_inner(ts1, ts2)
        }
        (CelValue::Duration(d1), CelValue::Duration(d2)) => temporal::duration_sub_inner(d1, d2),
        (a, b) => {
            error!(log, "Cannot subtract incompatible types";
                "left_type" => format!("{:?}", a),
                "right_type" => format!("{:?}", b));
            CelValue::Error("no such overload".into())
        }
    };
    Box::into_raw(Box::new(result))
}

/// Polymorphic multiplication operator.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_mul(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = unsafe { read_ptr(a_ptr) };
    let b = unsafe { read_ptr(b_ptr) };
    let log = crate::logging::get_logger();
    let result = match (a, b) {
        (CelValue::Error(e), _) => CelValue::Error(e),
        (_, CelValue::Error(e)) => CelValue::Error(e),
        (CelValue::Int(a), CelValue::Int(b)) => match a.checked_mul(b) {
            Some(r) => CelValue::Int(r),
            None => {
                error!(log, "Integer overflow in multiplication"; "left" => a, "right" => b);
                CelValue::Error("return error for overflow".into())
            }
        },
        (CelValue::UInt(a), CelValue::UInt(b)) => match a.checked_mul(b) {
            Some(r) => CelValue::UInt(r),
            None => {
                error!(log, "UInt overflow in multiplication"; "left" => a, "right" => b);
                CelValue::Error("return error for overflow".into())
            }
        },
        (CelValue::Double(a), CelValue::Double(b)) => {
            CelValue::Double(arithmetic::double_mul(a, b))
        }
        (a, b) => {
            error!(log, "Cannot multiply incompatible types";
                "left_type" => format!("{:?}", a), "right_type" => format!("{:?}", b));
            CelValue::Error("no such overload".into())
        }
    };
    Box::into_raw(Box::new(result))
}

/// Polymorphic division operator.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_div(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = unsafe { read_ptr(a_ptr) };
    let b = unsafe { read_ptr(b_ptr) };
    let log = crate::logging::get_logger();
    let result = match (a, b) {
        (CelValue::Error(e), _) => CelValue::Error(e),
        (_, CelValue::Error(e)) => CelValue::Error(e),
        (CelValue::Int(a), CelValue::Int(b)) => {
            if b == 0 {
                CelValue::Error("divide by zero".into())
            } else {
                match a.checked_div(b) {
                    Some(r) => CelValue::Int(r),
                    None => {
                        error!(log, "Integer overflow in division"; "dividend" => a, "divisor" => b);
                        CelValue::Error("return error for overflow".into())
                    }
                }
            }
        }
        (CelValue::UInt(a), CelValue::UInt(b)) => match a.checked_div(b) {
            Some(r) => CelValue::UInt(r),
            None => CelValue::Error("divide by zero".into()),
        },
        (CelValue::Double(a), CelValue::Double(b)) => {
            CelValue::Double(arithmetic::double_div(a, b))
        }
        (a, b) => {
            error!(log, "Cannot divide incompatible types";
                "left_type" => format!("{:?}", a), "right_type" => format!("{:?}", b));
            CelValue::Error("no such overload".into())
        }
    };
    Box::into_raw(Box::new(result))
}

/// Polymorphic modulo operator.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_mod(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = unsafe { read_ptr(a_ptr) };
    let b = unsafe { read_ptr(b_ptr) };
    let log = crate::logging::get_logger();
    let result = match (a, b) {
        (CelValue::Error(e), _) => CelValue::Error(e),
        (_, CelValue::Error(e)) => CelValue::Error(e),
        (CelValue::Int(a), CelValue::Int(b)) => {
            if b == 0 {
                CelValue::Error("modulus by zero".into())
            } else {
                match a.checked_rem(b) {
                    Some(r) => CelValue::Int(r),
                    None => CelValue::Error("return error for overflow".into()),
                }
            }
        }
        (CelValue::UInt(a), CelValue::UInt(b)) => {
            if b == 0 {
                CelValue::Error("modulus by zero".into())
            } else {
                CelValue::UInt(a % b)
            }
        }
        (a, b) => {
            error!(log, "Modulo is only defined for int and uint";
                "left_type" => format!("{:?}", a), "right_type" => format!("{:?}", b));
            CelValue::Error("no_such_overload".into())
        }
    };
    Box::into_raw(Box::new(result))
}

/// Internal helper function to check CEL equality between two CelValue references.
/// This implements CEL spec cross-type numeric equality.
/// Used by both the `==` operator and the `in` operator.
/// Check if a CelValue is a wrapper type and extract its wrapped value.
/// Returns Some(value) if it's a wrapper, None otherwise.
///
/// CEL wrapper types from google/protobuf/wrappers.proto:
/// - google.protobuf.BoolValue -> wraps bool
/// - google.protobuf.BytesValue -> wraps bytes
/// - google.protobuf.DoubleValue/FloatValue -> wraps double
/// - google.protobuf.Int32Value/Int64Value -> wraps int
/// - google.protobuf.StringValue -> wraps string
/// - google.protobuf.UInt32Value/UInt64Value -> wraps uint
///
/// CEL JSON value type (google/protobuf/struct.proto):
/// - google.protobuf.Value -> unwraps to the native CEL type of its active oneof kind
fn unwrap_if_wrapper(value: &CelValue) -> Option<CelValue> {
    if let CelValue::Object(map) = value {
        // Check if this is a wrapper type by looking at __type__ field
        use crate::types::CelMapKey;
        let type_key = CelMapKey::String("__type__".into());
        if let Some(CelValue::String(type_name)) = map.get(&type_key) {
            // Check if it's one of the 9 official wrapper types
            let is_wrapper = matches!(
                type_name.as_str(),
                "google.protobuf.BoolValue"
                    | "google.protobuf.BytesValue"
                    | "google.protobuf.DoubleValue"
                    | "google.protobuf.FloatValue"
                    | "google.protobuf.Int32Value"
                    | "google.protobuf.Int64Value"
                    | "google.protobuf.StringValue"
                    | "google.protobuf.UInt32Value"
                    | "google.protobuf.UInt64Value"
            );

            if is_wrapper {
                // Extract the "value" field (or return zero value if empty wrapper)
                let value_key = CelMapKey::String("value".into());
                if let Some(wrapped_value) = map.get(&value_key) {
                    return Some(wrapped_value.clone());
                } else {
                    // Empty wrapper - return zero value per CEL spec
                    return Some(get_wrapper_zero_value(type_name));
                }
            }

            // google.protobuf.Value is a JSON value type (struct.proto).
            // Its oneof `kind` field determines the native CEL type:
            //   number_value  -> double
            //   string_value  -> string
            //   bool_value    -> bool
            //   null_value    -> null  (also used when no kind is set)
            //   struct_value / list_value -> left as Object/Array (not unwrapped)
            if type_name == "google.protobuf.Value" {
                let number_key = CelMapKey::String("number_value".into());
                let string_key = CelMapKey::String("string_value".into());
                let bool_key = CelMapKey::String("bool_value".into());

                return Some(if let Some(v) = map.get(&number_key) {
                    v.clone()
                } else if let Some(v) = map.get(&string_key) {
                    v.clone()
                } else if let Some(v) = map.get(&bool_key) {
                    v.clone()
                } else {
                    // null_value is set, or no kind is set — both represent null
                    CelValue::Null
                });
            }
        }
    }
    None
}

/// Get the zero value for a wrapper type (per CEL spec, empty wrappers equal their zero value).
fn get_wrapper_zero_value(type_name: &str) -> CelValue {
    match type_name {
        "google.protobuf.BoolValue" => CelValue::Bool(false),
        "google.protobuf.BytesValue" => CelValue::Bytes(Vec::new()),
        "google.protobuf.DoubleValue" | "google.protobuf.FloatValue" => CelValue::Double(0.0),
        "google.protobuf.Int32Value" | "google.protobuf.Int64Value" => CelValue::Int(0),
        "google.protobuf.StringValue" => CelValue::String(String::new()),
        "google.protobuf.UInt32Value" | "google.protobuf.UInt64Value" => CelValue::UInt(0),
        _ => CelValue::Null, // Shouldn't happen
    }
}

pub(crate) fn cel_equals(a_val: &CelValue, b_val: &CelValue) -> bool {
    // Unwrap wrapper types before comparison (per CEL spec)
    let a_unwrapped = unwrap_if_wrapper(a_val).unwrap_or_else(|| a_val.clone());
    let b_unwrapped = unwrap_if_wrapper(b_val).unwrap_or_else(|| b_val.clone());
    let a_val = &a_unwrapped;
    let b_val = &b_unwrapped;
    match (a_val, b_val) {
        // Same-type comparisons
        (CelValue::Int(a), CelValue::Int(b)) => a == b,
        (CelValue::UInt(a), CelValue::UInt(b)) => a == b,
        (CelValue::Double(a), CelValue::Double(b)) => a == b,
        (CelValue::String(a), CelValue::String(b)) => a == b,
        (CelValue::Bool(a), CelValue::Bool(b)) => a == b,
        (CelValue::Bytes(a), CelValue::Bytes(b)) => a == b,
        (CelValue::Null, CelValue::Null) => true,
        (CelValue::Timestamp(a), CelValue::Timestamp(b)) => a == b,
        (CelValue::Duration(a), CelValue::Duration(b)) => a == b,
        (CelValue::Type(a), CelValue::Type(b)) => a == b,

        // Array comparison with cross-type numeric equality for elements
        (CelValue::Array(a), CelValue::Array(b)) => {
            if a.len() != b.len() {
                return false;
            }
            a.iter().zip(b.iter()).all(|(av, bv)| cel_equals(av, bv))
        }

        // Map comparison with key normalization and cross-type numeric equality for values
        (CelValue::Object(a), CelValue::Object(b)) => {
            // First, check if both are google.protobuf.Any objects and use schema-aware comparison
            if let Some(result) = crate::proto_wire::compare_any_objects(a, b) {
                return result;
            }

            if a.len() != b.len() {
                return false;
            }
            // For each key in map a, check if there's an equivalent key in map b with equal value
            for (a_key, a_value) in a.iter() {
                // Try to find the key in b, considering numeric key equivalence
                let mut found = false;
                for (b_key, b_value) in b.iter() {
                    if cel_map_keys_equal(a_key, b_key) {
                        if !cel_equals(a_value, b_value) {
                            return false;
                        }
                        found = true;
                        break;
                    }
                }
                if !found {
                    return false;
                }
            }
            true
        }

        // IP address equality: same-family uses direct comparison; cross-family
        // handles IPv4-mapped IPv6 (e.g. `::ffff:c0a8:1` == `192.168.0.1`).
        (CelValue::IpAddr(a), CelValue::IpAddr(b)) => {
            use std::net::IpAddr;
            match (a, b) {
                // Same family — direct equality (handles normalisation, e.g. IPv6 case).
                (IpAddr::V4(a4), IpAddr::V4(b4)) => a4 == b4,
                (IpAddr::V6(a6), IpAddr::V6(b6)) => a6 == b6,
                // Cross-family: V6 == V4 only if the V6 address is an IPv4-mapped IPv6.
                (IpAddr::V6(a6), IpAddr::V4(b4)) => a6.to_ipv4().as_ref() == Some(b4),
                (IpAddr::V4(a4), IpAddr::V6(b6)) => b6.to_ipv4().as_ref() == Some(a4),
            }
        }

        // CIDR equality: two CIDRs are equal if they have the same address and prefix length.
        // Note: host bits are NOT masked for equality — cidr('127.0.0.1/24') == cidr('127.0.0.1/24')
        // but cidr('127.0.0.1/24') != cidr('127.0.0.0/24').
        (CelValue::Cidr(a_addr, a_prefix), CelValue::Cidr(b_addr, b_prefix)) => {
            a_prefix == b_prefix && a_addr == b_addr
        }

        // Quantity equality: compare by numeric value
        (CelValue::Quantity(a), CelValue::Quantity(b)) => {
            crate::kubernetes::quantity::quantities_equal(a.as_str(), b.as_str())
        }

        // Semver equality: compare by precedence (ignoring build metadata, matching compareTo)
        (CelValue::Semver(a), CelValue::Semver(b)) => {
            a.cmp_precedence(b) == std::cmp::Ordering::Equal
        }

        // Optional equality: none == none, some(a) == some(b) iff a == b
        (CelValue::Optional(None), CelValue::Optional(None)) => true,
        (CelValue::Optional(Some(a)), CelValue::Optional(Some(b))) => cel_equals(a, b),
        (CelValue::Optional(_), CelValue::Optional(_)) => false,

        // Cross-type numeric equality (CEL spec: x == y if !(x < y || x > y))
        (CelValue::Int(a), CelValue::UInt(b)) => {
            if *a < 0 {
                false
            } else {
                (*a as u64) == *b
            }
        }
        (CelValue::UInt(a), CelValue::Int(b)) => {
            if *b < 0 {
                false
            } else {
                *a == (*b as u64)
            }
        }
        (CelValue::Int(a), CelValue::Double(b)) => (*a as f64) == *b,
        (CelValue::Double(a), CelValue::Int(b)) => *a == (*b as f64),
        (CelValue::UInt(a), CelValue::Double(b)) => (*a as f64) == *b,
        (CelValue::Double(a), CelValue::UInt(b)) => *a == (*b as f64),

        // Different types are not equal
        _ => false,
    }
}

/// Helper function to check if two map keys are equivalent considering numeric type conversions.
/// Per CEL spec, numeric keys (int, uint) should be compared with cross-type equality.
fn cel_map_keys_equal(a: &crate::types::CelMapKey, b: &crate::types::CelMapKey) -> bool {
    use crate::types::CelMapKey;
    match (a, b) {
        (CelMapKey::Bool(a), CelMapKey::Bool(b)) => a == b,
        (CelMapKey::String(a), CelMapKey::String(b)) => a == b,

        // Numeric key comparison with cross-type equality
        (CelMapKey::Int(a), CelMapKey::Int(b)) => a == b,
        (CelMapKey::UInt(a), CelMapKey::UInt(b)) => a == b,
        (CelMapKey::Int(a), CelMapKey::UInt(b)) => {
            if *a < 0 {
                false
            } else {
                (*a as u64) == *b
            }
        }
        (CelMapKey::UInt(a), CelMapKey::Int(b)) => {
            if *b < 0 {
                false
            } else {
                *a == (*b as u64)
            }
        }

        // Different key types are not equal
        _ => false,
    }
}

/// Polymorphic equality operator.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_eq(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = unsafe { read_ptr(a_ptr) };
    let b = unsafe { read_ptr(b_ptr) };
    let result = match (&a, &b) {
        (CelValue::Error(_), _) => return Box::into_raw(Box::new(a)),
        (_, CelValue::Error(_)) => return Box::into_raw(Box::new(b)),
        _ => cel_equals(&a, &b),
    };
    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Polymorphic inequality operator.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_ne(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = unsafe { read_ptr(a_ptr) };
    let b = unsafe { read_ptr(b_ptr) };
    let result = match (&a, &b) {
        (CelValue::Error(_), _) => return Box::into_raw(Box::new(a)),
        (_, CelValue::Error(_)) => return Box::into_raw(Box::new(b)),
        _ => !cel_equals(&a, &b),
    };
    Box::into_raw(Box::new(CelValue::Bool(result)))
}

/// Polymorphic greater-than operator.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_gt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = unsafe { read_ptr(a_ptr) };
    let b = unsafe { read_ptr(b_ptr) };
    match (&a, &b) {
        (CelValue::Error(_), _) => Box::into_raw(Box::new(a)),
        (_, CelValue::Error(_)) => Box::into_raw(Box::new(b)),
        _ => match cel_value_less_than(&b, &a) {
            Ok(result) => Box::into_raw(Box::new(CelValue::Bool(result))),
            Err(_) => {
                let log = crate::logging::get_logger();
                error!(log, "Cannot compare incompatible types for greater-than";
                    "left_type" => format!("{:?}", a), "right_type" => format!("{:?}", b));
                crate::error::create_error_value("no such overload")
            }
        },
    }
}

/// Polymorphic less-than operator.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_lt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = unsafe { read_ptr(a_ptr) };
    let b = unsafe { read_ptr(b_ptr) };
    match (&a, &b) {
        (CelValue::Error(_), _) => Box::into_raw(Box::new(a)),
        (_, CelValue::Error(_)) => Box::into_raw(Box::new(b)),
        _ => match cel_value_less_than(&a, &b) {
            Ok(result) => Box::into_raw(Box::new(CelValue::Bool(result))),
            Err(_) => {
                let log = crate::logging::get_logger();
                error!(log, "Cannot compare incompatible types for less-than";
                    "left_type" => format!("{:?}", a), "right_type" => format!("{:?}", b));
                crate::error::create_error_value("no such overload")
            }
        },
    }
}
pub(crate) fn cel_value_less_than(
    a_val: &CelValue,
    b_val: &CelValue,
) -> Result<bool, &'static str> {
    let result = match (a_val, b_val) {
        // Same-type comparisons
        (CelValue::Int(a), CelValue::Int(b)) => a < b,
        (CelValue::UInt(a), CelValue::UInt(b)) => a < b,
        (CelValue::Double(a), CelValue::Double(b)) => a < b,
        (CelValue::Bytes(a), CelValue::Bytes(b)) => a < b,
        (CelValue::String(a), CelValue::String(b)) => a < b,
        (CelValue::Bool(a), CelValue::Bool(b)) => a < b, // false < true in CEL
        (CelValue::Timestamp(a), CelValue::Timestamp(b)) => a < b,
        (CelValue::Duration(a), CelValue::Duration(b)) => a < b,

        // Cross-type numeric ordering
        (CelValue::Int(a), CelValue::UInt(b)) => {
            if *a < 0 {
                true
            } else {
                (*a as u64) < *b
            }
        }
        (CelValue::UInt(a), CelValue::Int(b)) => {
            if *b < 0 {
                false
            } else {
                *a < (*b as u64)
            }
        }
        (CelValue::Int(a), CelValue::Double(b)) => (*a as f64) < *b,
        (CelValue::Double(a), CelValue::Int(b)) => *a < (*b as f64),
        (CelValue::UInt(a), CelValue::Double(b)) => (*a as f64) < *b,
        (CelValue::Double(a), CelValue::UInt(b)) => *a < (*b as f64),

        _ => return Err("no such overload"),
    };
    Ok(result)
}

/// Polymorphic greater-than-or-equal operator.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_gte(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = unsafe { read_ptr(a_ptr) };
    let b = unsafe { read_ptr(b_ptr) };
    match (&a, &b) {
        (CelValue::Error(_), _) => Box::into_raw(Box::new(a)),
        (_, CelValue::Error(_)) => Box::into_raw(Box::new(b)),
        // gte(a, b) == !lt(a, b)
        _ => match cel_value_less_than(&a, &b) {
            Ok(lt) => Box::into_raw(Box::new(CelValue::Bool(!lt))),
            Err(_) => {
                let log = crate::logging::get_logger();
                error!(log, "Cannot compare incompatible types for >=";
                    "left_type" => format!("{:?}", a), "right_type" => format!("{:?}", b));
                crate::error::create_error_value("no such overload")
            }
        },
    }
}

/// Polymorphic less-than-or-equal operator.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_lte(
    a_ptr: *mut CelValue,
    b_ptr: *mut CelValue,
) -> *mut CelValue {
    let a = unsafe { read_ptr(a_ptr) };
    let b = unsafe { read_ptr(b_ptr) };
    match (&a, &b) {
        (CelValue::Error(_), _) => Box::into_raw(Box::new(a)),
        (_, CelValue::Error(_)) => Box::into_raw(Box::new(b)),
        // lte(a, b) == !lt(b, a)
        _ => match cel_value_less_than(&b, &a) {
            Ok(gt) => Box::into_raw(Box::new(CelValue::Bool(!gt))),
            Err(_) => {
                let log = crate::logging::get_logger();
                error!(log, "Cannot compare incompatible types for <=";
                    "left_type" => format!("{:?}", a), "right_type" => format!("{:?}", b));
                crate::error::create_error_value("no such overload")
            }
        },
    }
}

/// Polymorphic size function for CelValue objects.
/// Returns the size/length of the value:
/// - String: number of Unicode codepoints
/// - Bytes: number of bytes
/// - Array: number of elements
/// - Map: number of keys
///
/// # Safety
/// - `ptr` must be a valid, non-null CelValue pointer
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_size(ptr: *mut CelValue) -> i64 {
    let log = crate::logging::get_logger();

    let value = unsafe {
        if ptr.is_null() {
            error!(log, "Cannot get size of null value"; "function" => "cel_value_size");
            abort_with_error("no such overload");
        }
        &*ptr
    };

    match value {
        CelValue::String(_) => string::cel_string_size(ptr),
        CelValue::Bytes(_) => bytes::cel_bytes_size(ptr),
        CelValue::Array(arr) => arr.len() as i64,
        CelValue::Object(map) => map.len() as i64,
        other => {
            error!(log, "size() not supported for this type";
                "function" => "cel_value_size",
                "type" => format!("{:?}", other));
            abort_with_error("no such overload");
        }
    }
}

/// Polymorphic negation operator.
///
/// # Safety
/// Pointer must be valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_negate(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(ptr) };
    let log = crate::logging::get_logger();
    let result = match value {
        CelValue::Error(e) => CelValue::Error(e),
        CelValue::Int(i) => match i.checked_neg() {
            Some(r) => CelValue::Int(r),
            None => CelValue::Error("return error for overflow".into()),
        },
        CelValue::Double(d) => CelValue::Double(-d),
        CelValue::Duration(d) => temporal::duration_negate_inner(d),
        other => {
            error!(log, "Negation not supported for this type";
                "type" => format!("{:?}", other));
            CelValue::Error("no such overload".into())
        }
    };
    Box::into_raw(Box::new(result))
}

/// Index operator for arrays and maps.
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_value_index(
    container_ptr: *mut CelValue,
    index_ptr: *mut CelValue,
) -> *mut CelValue {
    let container = unsafe { read_ptr(container_ptr) };
    let index = unsafe { read_ptr(index_ptr) };
    let log = crate::logging::get_logger();
    Box::into_raw(Box::new(index_value(&log, container, index)))
}

/// Recursive index logic, kept as a separate function because it calls itself
/// for `Optional(Some(...))` containers.
fn index_value(log: &slog::Logger, container: CelValue, index: CelValue) -> CelValue {
    match (container, index) {
        // Error propagation
        (CelValue::Error(e), _) => CelValue::Error(e),
        (_, CelValue::Error(e)) => CelValue::Error(e),
        // Optional container
        (CelValue::Optional(None), _) => CelValue::Optional(None),
        (CelValue::Optional(Some(inner)), idx) => {
            let result = index_value(log, *inner, idx);
            match result {
                CelValue::Error(_) => CelValue::Optional(None),
                v => CelValue::Optional(Some(Box::new(v))),
            }
        }
        // Array indexing with Int
        (CelValue::Array(vec), CelValue::Int(idx)) => {
            let i = if idx < 0 {
                return CelValue::Error("index out of bounds".into());
            } else {
                idx as usize
            };
            match vec.into_iter().nth(i) {
                Some(v) => v,
                None => CelValue::Error("index out of bounds".into()),
            }
        }
        // Array indexing with UInt
        (CelValue::Array(vec), CelValue::UInt(idx)) => match vec.into_iter().nth(idx as usize) {
            Some(v) => v,
            None => CelValue::Error("index out of bounds".into()),
        },
        // Array indexing with Double (whole numbers only)
        (CelValue::Array(vec), CelValue::Double(idx)) => {
            if idx.fract() != 0.0 || idx < 0.0 {
                return CelValue::Error("no such overload".into());
            }
            match vec.into_iter().nth(idx as usize) {
                Some(v) => v,
                None => CelValue::Error("index out of bounds".into()),
            }
        }
        // Map indexing
        (CelValue::Object(map), key) => {
            use crate::types::CelMapKey;
            match CelMapKey::from_cel_value(&key) {
                Some(map_key) => {
                    debug!(log, "Indexing map"; "key" => map_key.to_string_key());
                    match map.get(&map_key) {
                        Some(value) => value.clone(),
                        None => CelValue::Error("no such key".into()),
                    }
                }
                None => {
                    error!(log, "Map key must be bool, int, uint, or string";
                        "key_type" => format!("{:?}", key));
                    CelValue::Error("no such overload".into())
                }
            }
        }
        // Array with wrong index type
        (CelValue::Array(_), idx) => {
            error!(log, "Array index must be Int, UInt, or Double";
                "index_type" => format!("{:?}", idx));
            CelValue::Error("no such overload".into())
        }
        // Anything else
        (container, index) => {
            error!(log, "Index operator not supported for this type";
                "container_type" => format!("{:?}", container),
                "index_type" => format!("{:?}", index));
            CelValue::Error("no such overload".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn extract_bool(ptr: *mut CelValue) -> bool {
        unsafe {
            match &*ptr {
                CelValue::Bool(b) => *b,
                other => panic!("expected Bool, got {:?}", other),
            }
        }
    }

    #[test]
    fn test_create_int() {
        unsafe {
            let ptr = Box::into_raw(Box::new(CelValue::Int(42)));
            assert_eq!(*ptr, CelValue::Int(42));
            // Clean up
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_bool_true() {
        unsafe {
            let ptr = cel_create_bool(1);
            assert_eq!(*ptr, CelValue::Bool(true));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_bool_false() {
        unsafe {
            let ptr = cel_create_bool(0);
            assert_eq!(*ptr, CelValue::Bool(false));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_bool_nonzero() {
        unsafe {
            let ptr = cel_create_bool(42);
            assert_eq!(*ptr, CelValue::Bool(true));
            let _ = Box::from_raw(ptr);
        }
    }

    // Integration tests for cel_value_add dispatcher

    #[test]
    fn test_extract_bool_true() {
        unsafe {
            let ptr = cel_create_bool(1);
            let value = extract_bool(ptr);
            assert!(value);
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_extract_bool_false() {
        unsafe {
            let ptr = cel_create_bool(0);
            let value = extract_bool(ptr);
            assert!(!value);
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_double() {
        unsafe {
            let ptr = Box::into_raw(Box::new(CelValue::Double(3.15)));
            assert_eq!(*ptr, CelValue::Double(3.15));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_double_negative() {
        unsafe {
            let ptr = Box::into_raw(Box::new(CelValue::Double(-2.5)));
            assert_eq!(*ptr, CelValue::Double(-2.5));
            let _ = Box::from_raw(ptr);
        }
    }

    // Integration tests for cel_value_add dispatcher

    #[rstest]
    #[case::int_add(CelValue::Int(2), CelValue::Int(3), CelValue::Int(5))]
    #[case::int_negative(CelValue::Int(-5), CelValue::Int(3), CelValue::Int(-2))]
    #[case::uint_add(CelValue::UInt(10), CelValue::UInt(20), CelValue::UInt(30))]
    #[case::uint_large(CelValue::UInt(u64::MAX - 100), CelValue::UInt(50), CelValue::UInt(u64::MAX - 50))]
    #[case::double_add(CelValue::Double(2.5), CelValue::Double(3.5), CelValue::Double(6.0))]
    #[case::double_negative(CelValue::Double(-5.5), CelValue::Double(3.0), CelValue::Double(-2.5))]
    #[case::string_basic(
        CelValue::String("hello".to_string()),
        CelValue::String(" world".to_string()),
        CelValue::String("hello world".to_string())
    )]
    #[case::string_empty(
        CelValue::String("".to_string()),
        CelValue::String("test".to_string()),
        CelValue::String("test".to_string())
    )]
    #[case::string_unicode(
        CelValue::String("Hello ".to_string()),
        CelValue::String("世界".to_string()),
        CelValue::String("Hello 世界".to_string())
    )]
    #[case::string_emoji(
        CelValue::String("Hello ".to_string()),
        CelValue::String("👋🌍".to_string()),
        CelValue::String("Hello 👋🌍".to_string())
    )]
    #[case::array_both_empty(
        CelValue::Array(vec![]),
        CelValue::Array(vec![]),
        CelValue::Array(vec![])
    )]
    #[case::array_basic(
        CelValue::Array(vec![CelValue::Int(1), CelValue::Int(2)]),
        CelValue::Array(vec![CelValue::Int(3), CelValue::Int(4)]),
        CelValue::Array(vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3), CelValue::Int(4)])
    )]
    #[case::array_mixed(
        CelValue::Array(vec![CelValue::Int(1), CelValue::Bool(true)]),
        CelValue::Array(vec![CelValue::Int(2)]),
        CelValue::Array(vec![CelValue::Int(1), CelValue::Bool(true), CelValue::Int(2)])
    )]
    fn test_value_add(#[case] a: CelValue, #[case] b: CelValue, #[case] expected: CelValue) {
        let a_ptr = Box::into_raw(Box::new(a));
        let b_ptr = Box::into_raw(Box::new(b));

        unsafe {
            let result_ptr = cel_value_add(a_ptr, b_ptr);
            let result = &*result_ptr;

            assert_eq!(result, &expected);
        }
    }

    // Uint creation and extraction tests

    #[test]
    fn test_create_uint() {
        unsafe {
            let ptr = Box::into_raw(Box::new(CelValue::UInt(123)));
            assert_eq!(*ptr, CelValue::UInt(123));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_uint_max() {
        unsafe {
            let ptr = Box::into_raw(Box::new(CelValue::UInt(u64::MAX)));
            assert_eq!(*ptr, CelValue::UInt(u64::MAX));
            let _ = Box::from_raw(ptr);
        }
    }

    // Cross-type equality tests

    #[rstest]
    #[case::int_uint_equal(CelValue::Int(1), CelValue::UInt(1), CelValue::Bool(true))]
    #[case::int_uint_different(CelValue::Int(1), CelValue::UInt(2), CelValue::Bool(false))]
    #[case::int_negative_uint(CelValue::Int(-1), CelValue::UInt(1), CelValue::Bool(false))]
    #[case::int_double_equal(CelValue::Int(5), CelValue::Double(5.0), CelValue::Bool(true))]
    #[case::int_double_different(CelValue::Int(5), CelValue::Double(5.5), CelValue::Bool(false))]
    #[case::uint_double_equal(CelValue::UInt(10), CelValue::Double(10.0), CelValue::Bool(true))]
    #[case::uint_double_different(
        CelValue::UInt(10),
        CelValue::Double(10.5),
        CelValue::Bool(false)
    )]
    #[case::uint_uint_equal(CelValue::UInt(100), CelValue::UInt(100), CelValue::Bool(true))]
    #[case::uint_uint_different(CelValue::UInt(100), CelValue::UInt(200), CelValue::Bool(false))]
    fn test_cross_type_equality(
        #[case] a: CelValue,
        #[case] b: CelValue,
        #[case] expected: CelValue,
    ) {
        let a_ptr = Box::into_raw(Box::new(a));
        let b_ptr = Box::into_raw(Box::new(b));

        unsafe {
            let result_ptr = cel_value_eq(a_ptr, b_ptr);
            let result = &*result_ptr;

            assert_eq!(result, &expected);
        }
    }

    // Cross-type ordering tests

    #[rstest]
    #[case::int_negative_lt_uint(CelValue::Int(-1), CelValue::UInt(1), CelValue::Bool(true))]
    #[case::int_positive_lt_uint(CelValue::Int(5), CelValue::UInt(10), CelValue::Bool(true))]
    #[case::int_gt_uint(CelValue::Int(10), CelValue::UInt(5), CelValue::Bool(false))]
    #[case::int_lt_double(CelValue::Int(5), CelValue::Double(10.0), CelValue::Bool(true))]
    #[case::uint_lt_double(CelValue::UInt(5), CelValue::Double(10.0), CelValue::Bool(true))]
    #[case::uint_gt_double(CelValue::UInt(100), CelValue::Double(50.0), CelValue::Bool(false))]
    #[case::uint_lt_uint(CelValue::UInt(50), CelValue::UInt(100), CelValue::Bool(true))]
    fn test_cross_type_less_than(
        #[case] a: CelValue,
        #[case] b: CelValue,
        #[case] expected: CelValue,
    ) {
        let a_ptr = Box::into_raw(Box::new(a));
        let b_ptr = Box::into_raw(Box::new(b));

        unsafe {
            let result_ptr = cel_value_lt(a_ptr, b_ptr);
            let result = &*result_ptr;

            assert_eq!(result, &expected);
        }
    }

    #[rstest]
    #[case::int_negative_lt_uint(CelValue::Int(-1), CelValue::UInt(1), CelValue::Bool(false))]
    #[case::int_positive_gt_uint(CelValue::Int(10), CelValue::UInt(5), CelValue::Bool(true))]
    #[case::int_lt_uint(CelValue::Int(5), CelValue::UInt(10), CelValue::Bool(false))]
    #[case::int_gt_double(CelValue::Int(10), CelValue::Double(5.0), CelValue::Bool(true))]
    #[case::uint_gt_double(CelValue::UInt(100), CelValue::Double(50.0), CelValue::Bool(true))]
    #[case::uint_lt_double(CelValue::UInt(5), CelValue::Double(10.0), CelValue::Bool(false))]
    #[case::uint_gt_uint(CelValue::UInt(100), CelValue::UInt(50), CelValue::Bool(true))]
    fn test_cross_type_greater_than(
        #[case] a: CelValue,
        #[case] b: CelValue,
        #[case] expected: CelValue,
    ) {
        let a_ptr = Box::into_raw(Box::new(a));
        let b_ptr = Box::into_raw(Box::new(b));

        unsafe {
            let result_ptr = cel_value_gt(a_ptr, b_ptr);
            let result = &*result_ptr;

            assert_eq!(result, &expected);
        }
    }
}
