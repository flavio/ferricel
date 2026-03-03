//! Helper functions for creating and extracting CelValue pointers.
//! These are used internally by other runtime functions and exported for compiler use.
//! Also includes polymorphic operators that dispatch to type-specific implementations.

use crate::error::abort_with_error;
use crate::types::CelValue;
use crate::{arithmetic, array, bytes, string, temporal};
use slog::{debug, error};

/// Creates a CelValue::Int on the heap and returns a pointer to it.
/// The caller is responsible for freeing the memory using cel_free_value.
#[unsafe(no_mangle)]
pub extern "C" fn cel_create_int(value: i64) -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::Int(value)))
}

/// Creates a CelValue::UInt on the heap and returns a pointer to it.
/// The caller is responsible for freeing the memory using cel_free_value.
#[unsafe(no_mangle)]
pub extern "C" fn cel_create_uint(value: u64) -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::UInt(value)))
}

/// Creates a CelValue::Bool on the heap and returns a pointer to it.
/// Input: i64 where 0 = false, non-zero = true
/// The caller is responsible for freeing the memory using cel_free_value.
#[unsafe(no_mangle)]
pub extern "C" fn cel_create_bool(value: i64) -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::Bool(value != 0)))
}

/// Creates a CelValue::Double on the heap and returns a pointer to it.
/// The caller is responsible for freeing the memory using cel_free_value.
#[unsafe(no_mangle)]
pub extern "C" fn cel_create_double(value: f64) -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::Double(value)))
}

/// Creates a CelValue::Timestamp on the heap and returns a pointer to it.
/// The caller is responsible for freeing the memory using cel_free_value.
///
/// # Arguments
/// * `seconds` - Seconds since Unix epoch (1970-01-01T00:00:00Z)
/// * `nanos` - Nanoseconds component (0-999,999,999)
#[unsafe(no_mangle)]
pub extern "C" fn cel_create_timestamp(seconds: i64, nanos: i64) -> *mut CelValue {
    let dt = crate::chrono_helpers::parts_to_datetime(seconds, nanos);
    Box::into_raw(Box::new(CelValue::Timestamp(dt)))
}

/// Creates a CelValue::Duration on the heap and returns a pointer to it.
/// The caller is responsible for freeing the memory using cel_free_value.
///
/// # Arguments
/// * `seconds` - Number of seconds (can be negative)
/// * `nanos` - Nanoseconds component (0-999,999,999 or negative)
#[unsafe(no_mangle)]
pub extern "C" fn cel_create_duration(seconds: i64, nanos: i64) -> *mut CelValue {
    let duration = crate::chrono_helpers::parts_to_duration(seconds, nanos);
    Box::into_raw(Box::new(CelValue::Duration(duration)))
}

/// Creates a CelValue::Null on the heap and returns a pointer to it.
/// The caller is responsible for freeing the memory using cel_free_value.
#[unsafe(no_mangle)]
pub extern "C" fn cel_create_null() -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::Null))
}

/// Creates a CelValue::Type on the heap from a string pointer and returns a pointer to it.
/// The caller is responsible for freeing the memory using cel_free_value.
///
/// # Arguments
/// * `type_name_ptr` - Pointer to the type name string in WASM memory
/// * `type_name_len` - Length of the type name string
#[unsafe(no_mangle)]
pub extern "C" fn cel_create_type(type_name_ptr: *const u8, type_name_len: i32) -> *mut CelValue {
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

/// Internal helper: Extracts i64 from CelValue or panics with type error.
/// This is not exported - it's used by arithmetic and comparison operations.
pub(crate) fn extract_int(ptr: *mut CelValue) -> i64 {
    let log = crate::logging::get_logger();
    extract_int_with_log(ptr, &log)
}

/// Internal helper with logger: Extracts i64 from CelValue or aborts with structured error.
pub(crate) fn extract_int_with_log(ptr: *mut CelValue, log: &slog::Logger) -> i64 {
    unsafe {
        if ptr.is_null() {
            error!(log, "Null pointer in extract operation";
                "function" => "extract_int",
                "pointer" => "null");
            abort_with_error("no such overload");
        }
        match &*ptr {
            CelValue::Int(i) => *i,
            other => {
                error!(log, "Type mismatch in extraction";
                    "function" => "extract_int",
                    "expected" => "Int",
                    "actual" => format!("{:?}", other));
                abort_with_error("no such overload");
            }
        }
    }
}

/// Internal helper: Extracts u64 from CelValue or panics with type error.
/// This is not exported - it's used by arithmetic and comparison operations.
pub(crate) fn extract_uint(ptr: *mut CelValue) -> u64 {
    let log = crate::logging::get_logger();
    extract_uint_with_log(ptr, &log)
}

/// Internal helper with logger: Extracts u64 from CelValue or aborts with structured error.
pub(crate) fn extract_uint_with_log(ptr: *mut CelValue, log: &slog::Logger) -> u64 {
    unsafe {
        if ptr.is_null() {
            error!(log, "Null pointer in extract operation";
                "function" => "extract_uint",
                "pointer" => "null");
            abort_with_error("no such overload");
        }
        match &*ptr {
            CelValue::UInt(u) => *u,
            other => {
                error!(log, "Type mismatch in extraction";
                    "function" => "extract_uint",
                    "expected" => "UInt",
                    "actual" => format!("{:?}", other));
                abort_with_error("no such overload");
            }
        }
    }
}

/// Internal helper: Extracts bool from CelValue or panics with type error.
/// This is not exported - it's used by logical operations.
pub(crate) fn extract_bool(ptr: *mut CelValue) -> bool {
    let log = crate::logging::get_logger();
    extract_bool_with_log(ptr, &log)
}

/// Internal helper with logger: Extracts bool from CelValue or aborts with structured error.
pub(crate) fn extract_bool_with_log(ptr: *mut CelValue, log: &slog::Logger) -> bool {
    unsafe {
        if ptr.is_null() {
            error!(log, "Null pointer in extract operation";
                "function" => "extract_bool",
                "pointer" => "null");
            abort_with_error("no such overload");
        }
        match &*ptr {
            CelValue::Bool(b) => *b,
            other => {
                error!(log, "Type mismatch in extraction";
                    "function" => "extract_bool",
                    "expected" => "Bool",
                    "actual" => format!("{:?}", other));
                abort_with_error("no such overload");
            }
        }
    }
}

/// Internal helper: Extracts f64 from CelValue or panics with type error.
/// This is not exported - it's used by arithmetic and comparison operations.
pub(crate) fn extract_double(ptr: *mut CelValue) -> f64 {
    let log = crate::logging::get_logger();
    extract_double_with_log(ptr, &log)
}

/// Internal helper with logger: Extracts f64 from CelValue or aborts with structured error.
pub(crate) fn extract_double_with_log(ptr: *mut CelValue, log: &slog::Logger) -> f64 {
    unsafe {
        if ptr.is_null() {
            error!(log, "Null pointer in extract operation";
                "function" => "extract_double",
                "pointer" => "null");
            abort_with_error("no such overload");
        }
        match &*ptr {
            CelValue::Double(d) => *d,
            other => {
                error!(log, "Type mismatch in extraction";
                    "function" => "extract_double",
                    "expected" => "Double",
                    "actual" => format!("{:?}", other));
                abort_with_error("no such overload");
            }
        }
    }
}

/// Internal helper: Extracts (seconds, nanos) from CelValue::Duration or panics with type error.
/// This is not exported - it's used by temporal operations.
pub(crate) fn extract_duration(ptr: *mut CelValue) -> (i64, i32) {
    let log = crate::logging::get_logger();
    extract_duration_with_log(ptr, &log)
}

/// Internal helper with logger: Extracts (seconds, nanos) from CelValue::Duration.
pub(crate) fn extract_duration_with_log(ptr: *mut CelValue, log: &slog::Logger) -> (i64, i32) {
    unsafe {
        if ptr.is_null() {
            error!(log, "Null pointer in extract operation";
                "function" => "extract_duration",
                "pointer" => "null");
            abort_with_error("no such overload");
        }
        match &*ptr {
            CelValue::Duration(d) => crate::chrono_helpers::duration_to_parts(d),
            other => {
                error!(log, "Type mismatch in extraction";
                    "function" => "extract_duration",
                    "expected" => "Duration",
                    "actual" => format!("{:?}", other));
                abort_with_error("no such overload");
            }
        }
    }
}

/// Internal helper: Extracts DateTime<FixedOffset> directly from CelValue::Timestamp.
/// This is not exported - it's used by temporal accessor operations.
pub(crate) fn extract_datetime(ptr: *mut CelValue) -> chrono::DateTime<chrono::FixedOffset> {
    let log = crate::logging::get_logger();
    extract_datetime_with_log(ptr, &log)
}

/// Internal helper with logger: Extracts DateTime<FixedOffset> from CelValue::Timestamp.
pub(crate) fn extract_datetime_with_log(
    ptr: *mut CelValue,
    log: &slog::Logger,
) -> chrono::DateTime<chrono::FixedOffset> {
    unsafe {
        if ptr.is_null() {
            error!(log, "Null pointer in extract operation";
                "function" => "extract_datetime",
                "pointer" => "null");
            abort_with_error("no such overload");
        }
        match &*ptr {
            CelValue::Timestamp(dt) => *dt,
            other => {
                error!(log, "Type mismatch in extraction";
                    "function" => "extract_datetime",
                    "expected" => "Timestamp",
                    "actual" => format!("{:?}", other));
                abort_with_error("no such overload");
            }
        }
    }
}

/// Internal helper: Extracts String from CelValue::String.
/// This is not exported - it's used by timezone-aware temporal operations.
pub(crate) fn extract_string(ptr: *mut CelValue) -> String {
    let log = crate::logging::get_logger();
    extract_string_with_log(ptr, &log)
}

/// Internal helper with logger: Extracts String from CelValue::String.
pub(crate) fn extract_string_with_log(ptr: *mut CelValue, log: &slog::Logger) -> String {
    unsafe {
        if ptr.is_null() {
            error!(log, "Null pointer in extract operation";
                "function" => "extract_string",
                "pointer" => "null");
            abort_with_error("no such overload");
        }
        match &*ptr {
            CelValue::String(s) => s.clone(),
            other => {
                error!(log, "Type mismatch in extraction";
                    "function" => "extract_string",
                    "expected" => "String",
                    "actual" => format!("{:?}", other));
                abort_with_error("no such overload");
            }
        }
    }
}

/// Internal helper: Extracts chrono::Duration directly from CelValue::Duration.
/// This is not exported - it's used by temporal arithmetic operations.
pub(crate) fn extract_duration_chrono(ptr: *mut CelValue) -> chrono::Duration {
    let log = crate::logging::get_logger();
    extract_duration_chrono_with_log(ptr, &log)
}

/// Internal helper with logger: Extracts chrono::Duration from CelValue::Duration.
pub(crate) fn extract_duration_chrono_with_log(
    ptr: *mut CelValue,
    log: &slog::Logger,
) -> chrono::Duration {
    unsafe {
        if ptr.is_null() {
            error!(log, "Null pointer in extract operation";
                "function" => "extract_duration_chrono",
                "pointer" => "null");
            abort_with_error("no such overload");
        }
        match &*ptr {
            CelValue::Duration(d) => *d,
            other => {
                error!(log, "Type mismatch in extraction";
                    "function" => "extract_duration_chrono",
                    "expected" => "Duration",
                    "actual" => format!("{:?}", other));
                abort_with_error("no such overload");
            }
        }
    }
}

/// Internal helper: Extracts chrono::DateTime from CelValue::Timestamp.
/// This is not exported - it's used by temporal comparison operations.
pub(crate) fn extract_timestamp(ptr: *mut CelValue) -> chrono::DateTime<chrono::FixedOffset> {
    let log = crate::logging::get_logger();
    extract_timestamp_with_log(ptr, &log)
}

/// Internal helper with logger: Extracts chrono::DateTime from CelValue::Timestamp.
pub(crate) fn extract_timestamp_with_log(
    ptr: *mut CelValue,
    log: &slog::Logger,
) -> chrono::DateTime<chrono::FixedOffset> {
    unsafe {
        if ptr.is_null() {
            error!(log, "Null pointer in extract operation";
                "function" => "extract_timestamp",
                "pointer" => "null");
            abort_with_error("no such overload");
        }
        match &*ptr {
            CelValue::Timestamp(dt) => *dt,
            other => {
                error!(log, "Type mismatch in extraction";
                    "function" => "extract_timestamp",
                    "expected" => "Timestamp",
                    "actual" => format!("{:?}", other));
                abort_with_error("no such overload");
            }
        }
    }
}

/// Polymorphic addition operator for CelValue objects.
/// Dispatches to type-specific implementations:
/// - Int + Int = Int (arithmetic addition)
/// - Double + Double = Double (arithmetic addition)
/// - String + String = String (concatenation)
/// - Array + Array = Array (concatenation)
///
/// Note: Following CEL spec, there is NO automatic type coercion.
/// Mixed-type arithmetic (e.g., Int + Double) will panic.
///
/// # Safety
/// - Both pointers must be valid, non-null CelValue pointers
///
/// # Arguments
/// - `a_ptr`: Pointer to the first operand
/// - `b_ptr`: Pointer to the second operand
///
/// # Returns
/// Pointer to a new heap-allocated CelValue containing the result
///
/// # Panics
/// - If either pointer is null
/// - If the operand types don't match
/// - If the operation is not supported for the given types
/// - On integer overflow (for Int addition)
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_add(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if a_ptr.is_null() || b_ptr.is_null() {
            error!(log, "Cannot add null values";
                "function" => "cel_value_add");
            abort_with_error("no such overload");
        }

        let a_val = &*a_ptr;
        let b_val = &*b_ptr;

        match (a_val, b_val) {
            (CelValue::Int(a), CelValue::Int(b)) => {
                debug!(log, "Performing Int addition"; "left" => *a, "right" => *b);
                let result = arithmetic::cel_int_add(*a, *b);
                cel_create_int(result)
            }
            (CelValue::UInt(a), CelValue::UInt(b)) => {
                debug!(log, "Performing UInt addition"; "left" => *a, "right" => *b);
                match a.checked_add(*b) {
                    Some(result) => cel_create_uint(result),
                    None => {
                        error!(log, "Unsigned integer overflow in addition";
                            "operation" => "cel_value_add",
                            "type" => "UInt",
                            "left" => *a,
                            "right" => *b);
                        abort_with_error("return error for overflow");
                    }
                }
            }
            (CelValue::Double(a), CelValue::Double(b)) => {
                debug!(log, "Performing Double addition"; "left" => *a, "right" => *b);
                let result = arithmetic::double_add(*a, *b);
                cel_create_double(result)
            }
            (CelValue::String(a_str), CelValue::String(b_str)) => {
                debug!(log, "Performing String concatenation"; 
                    "left_len" => a_str.len(), "right_len" => b_str.len());
                let result = string::cel_string_concat(a_str, b_str);
                Box::into_raw(Box::new(CelValue::String(result)))
            }
            (CelValue::Bytes(a_bytes), CelValue::Bytes(b_bytes)) => {
                debug!(log, "Performing Bytes concatenation"; 
                    "left_len" => a_bytes.len(), "right_len" => b_bytes.len());
                let result = bytes::cel_bytes_concat_internal(a_bytes, b_bytes);
                Box::into_raw(Box::new(CelValue::Bytes(result)))
            }
            (CelValue::Array(a_vec), CelValue::Array(b_vec)) => {
                debug!(log, "Performing Array concatenation"; 
                    "left_len" => a_vec.len(), "right_len" => b_vec.len());
                let result = array::cel_array_concat(a_vec, b_vec);
                Box::into_raw(Box::new(CelValue::Array(result)))
            }
            (CelValue::Timestamp(_), CelValue::Duration(_)) => {
                debug!(log, "Performing Timestamp + Duration");
                temporal::cel_timestamp_add_duration(a_ptr, b_ptr)
            }
            (CelValue::Duration(_), CelValue::Timestamp(_)) => {
                debug!(log, "Performing Duration + Timestamp");
                temporal::cel_timestamp_add_duration(b_ptr, a_ptr)
            }
            (CelValue::Duration(_), CelValue::Duration(_)) => {
                debug!(log, "Performing Duration + Duration");
                temporal::cel_duration_add(a_ptr, b_ptr)
            }
            _ => {
                error!(log, "Cannot add incompatible types";
                    "operation" => "cel_value_add",
                    "left_type" => format!("{:?}", a_val),
                    "right_type" => format!("{:?}", b_val));
                abort_with_error("no such overload");
            }
        }
    }
}

/// Polymorphic subtraction operator for CelValue objects.
/// Dispatches to type-specific implementations based on operand types.
///
/// Note: Following CEL spec, there is NO automatic type coercion.
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_sub(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if a_ptr.is_null() || b_ptr.is_null() {
            error!(log, "Cannot subtract null values";
                "function" => "cel_value_sub");
            abort_with_error("no such overload");
        }

        let a_val = &*a_ptr;
        let b_val = &*b_ptr;

        match (a_val, b_val) {
            (CelValue::Int(a), CelValue::Int(b)) => {
                debug!(log, "Performing Int subtraction"; "left" => *a, "right" => *b);
                match a.checked_sub(*b) {
                    Some(result) => cel_create_int(result),
                    None => {
                        error!(log, "Integer overflow in subtraction";
                            "operation" => "cel_value_sub",
                            "type" => "Int",
                            "left" => *a,
                            "right" => *b);
                        abort_with_error("return error for overflow");
                    }
                }
            }
            (CelValue::UInt(a), CelValue::UInt(b)) => {
                debug!(log, "Performing UInt subtraction"; "left" => *a, "right" => *b);
                match a.checked_sub(*b) {
                    Some(result) => cel_create_uint(result),
                    None => {
                        error!(log, "Unsigned integer underflow in subtraction";
                            "operation" => "cel_value_sub",
                            "type" => "UInt",
                            "left" => *a,
                            "right" => *b);
                        abort_with_error("return error for overflow");
                    }
                }
            }
            (CelValue::Double(a), CelValue::Double(b)) => {
                debug!(log, "Performing Double subtraction"; "left" => *a, "right" => *b);
                let result = arithmetic::double_sub(*a, *b);
                cel_create_double(result)
            }
            (CelValue::Timestamp(_), CelValue::Duration(_)) => {
                debug!(log, "Performing Timestamp - Duration");
                temporal::cel_timestamp_sub_duration(a_ptr, b_ptr)
            }
            (CelValue::Timestamp(_), CelValue::Timestamp(_)) => {
                debug!(log, "Performing Timestamp - Timestamp");
                temporal::cel_timestamp_diff(a_ptr, b_ptr)
            }
            (CelValue::Duration(_), CelValue::Duration(_)) => {
                debug!(log, "Performing Duration - Duration");
                temporal::cel_duration_sub(a_ptr, b_ptr)
            }
            _ => {
                error!(log, "Cannot subtract incompatible types";
                    "operation" => "cel_value_sub",
                    "left_type" => format!("{:?}", a_val),
                    "right_type" => format!("{:?}", b_val));
                abort_with_error("no such overload");
            }
        }
    }
}

/// Polymorphic multiplication operator for CelValue objects.
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_mul(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if a_ptr.is_null() || b_ptr.is_null() {
            error!(log, "Cannot multiply null values";
                "function" => "cel_value_mul");
            abort_with_error("no such overload");
        }

        let a_val = &*a_ptr;
        let b_val = &*b_ptr;

        match (a_val, b_val) {
            (CelValue::Int(a), CelValue::Int(b)) => {
                debug!(log, "Performing Int multiplication"; "left" => *a, "right" => *b);
                match a.checked_mul(*b) {
                    Some(result) => cel_create_int(result),
                    None => {
                        error!(log, "Integer overflow in multiplication";
                            "operation" => "cel_value_mul",
                            "type" => "Int",
                            "left" => *a,
                            "right" => *b);
                        abort_with_error("return error for overflow");
                    }
                }
            }
            (CelValue::UInt(a), CelValue::UInt(b)) => {
                debug!(log, "Performing UInt multiplication"; "left" => *a, "right" => *b);
                match a.checked_mul(*b) {
                    Some(result) => cel_create_uint(result),
                    None => {
                        error!(log, "Unsigned integer overflow in multiplication";
                            "operation" => "cel_value_mul",
                            "type" => "UInt",
                            "left" => *a,
                            "right" => *b);
                        abort_with_error("return error for overflow");
                    }
                }
            }
            (CelValue::Double(a), CelValue::Double(b)) => {
                debug!(log, "Performing Double multiplication"; "left" => *a, "right" => *b);
                let result = arithmetic::double_mul(*a, *b);
                cel_create_double(result)
            }
            _ => {
                error!(log, "Cannot multiply incompatible types";
                    "operation" => "cel_value_mul",
                    "left_type" => format!("{:?}", a_val),
                    "right_type" => format!("{:?}", b_val));
                abort_with_error("no such overload");
            }
        }
    }
}

/// Polymorphic division operator for CelValue objects.
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_div(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if a_ptr.is_null() || b_ptr.is_null() {
            error!(log, "Cannot divide null values";
                "function" => "cel_value_div");
            abort_with_error("no such overload");
        }

        let a_val = &*a_ptr;
        let b_val = &*b_ptr;

        match (a_val, b_val) {
            (CelValue::Int(a), CelValue::Int(b)) => {
                debug!(log, "Performing Int division"; "dividend" => *a, "divisor" => *b);
                if *b == 0 {
                    error!(log, "Division by zero";
                        "operation" => "cel_value_div",
                        "type" => "Int",
                        "dividend" => *a,
                        "divisor" => *b);
                    abort_with_error("divide by zero");
                }
                match a.checked_div(*b) {
                    Some(result) => cel_create_int(result),
                    None => {
                        error!(log, "Integer overflow in division";
                            "operation" => "cel_value_div",
                            "dividend" => *a,
                            "divisor" => *b);
                        abort_with_error("return error for overflow");
                    }
                }
            }
            (CelValue::UInt(a), CelValue::UInt(b)) => {
                debug!(log, "Performing UInt division"; "dividend" => *a, "divisor" => *b);
                if *b == 0 {
                    error!(log, "Division by zero";
                        "operation" => "cel_value_div",
                        "type" => "UInt",
                        "dividend" => *a,
                        "divisor" => *b);
                    abort_with_error("divide by zero");
                }
                cel_create_uint(a / b)
            }
            (CelValue::Double(a), CelValue::Double(b)) => {
                debug!(log, "Performing Double division"; "dividend" => *a, "divisor" => *b);
                let result = arithmetic::double_div(*a, *b);
                cel_create_double(result)
            }
            _ => {
                error!(log, "Cannot divide incompatible types";
                    "operation" => "cel_value_div",
                    "left_type" => format!("{:?}", a_val),
                    "right_type" => format!("{:?}", b_val));
                abort_with_error("no such overload");
            }
        }
    }
}

/// Polymorphic modulo operator for CelValue objects.
/// Note: Per CEL spec, modulo is only defined for int and uint.
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_mod(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if a_ptr.is_null() || b_ptr.is_null() {
            error!(log, "Cannot modulo null values";
                "function" => "cel_value_mod");
            abort_with_error("no such overload");
        }

        let a_val = &*a_ptr;
        let b_val = &*b_ptr;

        match (a_val, b_val) {
            (CelValue::Int(a), CelValue::Int(b)) => {
                debug!(log, "Performing Int modulo"; "dividend" => *a, "divisor" => *b);
                if *b == 0 {
                    error!(log, "Modulo by zero";
                        "operation" => "cel_value_mod",
                        "type" => "Int",
                        "dividend" => *a,
                        "divisor" => *b);
                    abort_with_error("modulus by zero");
                }
                match a.checked_rem(*b) {
                    Some(result) => cel_create_int(result),
                    None => abort_with_error("return error for overflow"),
                }
            }
            (CelValue::UInt(a), CelValue::UInt(b)) => {
                debug!(log, "Performing UInt modulo"; "dividend" => *a, "divisor" => *b);
                if *b == 0 {
                    error!(log, "Modulo by zero";
                        "operation" => "cel_value_mod",
                        "type" => "UInt",
                        "dividend" => *a,
                        "divisor" => *b);
                    abort_with_error("modulus by zero");
                }
                cel_create_uint(a % b)
            }
            _ => {
                error!(log, "Modulo is only defined for int and uint";
                    "operation" => "cel_value_mod",
                    "left_type" => format!("{:?}", a_val),
                    "right_type" => format!("{:?}", b_val));
                abort_with_error("no such overload")
            }
        }
    }
}

/// Internal helper function to check CEL equality between two CelValue references.
/// This implements CEL spec cross-type numeric equality.
/// Used by both the `==` operator and the `in` operator.
pub(crate) fn cel_equals(a_val: &CelValue, b_val: &CelValue) -> bool {
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

/// Polymorphic equality operator for CelValue objects.
/// Implements CEL spec cross-type numeric equality: int, uint, and double
/// are compared as if they exist on a continuous number line.
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_eq(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if a_ptr.is_null() || b_ptr.is_null() {
            error!(log, "Cannot compare null values";
                "function" => "cel_value_eq");
            abort_with_error("no such overload");
        }

        let a_val = &*a_ptr;
        let b_val = &*b_ptr;

        let result = cel_equals(a_val, b_val);
        cel_create_bool(if result { 1 } else { 0 })
    }
}

/// Polymorphic inequality operator for CelValue objects.
/// Implements CEL spec cross-type numeric inequality (negation of equality).
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_ne(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if a_ptr.is_null() || b_ptr.is_null() {
            error!(log, "Cannot compare null values";
                "function" => "cel_value_ne");
            abort_with_error("no such overload");
        }

        let a_val = &*a_ptr;
        let b_val = &*b_ptr;

        let result = match (a_val, b_val) {
            // Same-type comparisons
            (CelValue::Int(a), CelValue::Int(b)) => a != b,
            (CelValue::UInt(a), CelValue::UInt(b)) => a != b,
            (CelValue::Double(a), CelValue::Double(b)) => a != b,
            (CelValue::String(a), CelValue::String(b)) => a != b,
            (CelValue::Bool(a), CelValue::Bool(b)) => a != b,
            (CelValue::Bytes(a), CelValue::Bytes(b)) => a != b,
            (CelValue::Null, CelValue::Null) => false,
            (CelValue::Array(a), CelValue::Array(b)) => a != b,
            (CelValue::Object(a), CelValue::Object(b)) => a != b,
            (CelValue::Timestamp(a), CelValue::Timestamp(b)) => a != b,
            (CelValue::Duration(a), CelValue::Duration(b)) => a != b,
            (CelValue::Type(a), CelValue::Type(b)) => a != b,

            // Cross-type numeric inequality
            (CelValue::Int(a), CelValue::UInt(b)) => {
                if *a < 0 {
                    true
                } else {
                    (*a as u64) != *b
                }
            }
            (CelValue::UInt(a), CelValue::Int(b)) => {
                if *b < 0 {
                    true
                } else {
                    *a != (*b as u64)
                }
            }
            (CelValue::Int(a), CelValue::Double(b)) => (*a as f64) != *b,
            (CelValue::Double(a), CelValue::Int(b)) => *a != (*b as f64),
            (CelValue::UInt(a), CelValue::Double(b)) => (*a as f64) != *b,
            (CelValue::Double(a), CelValue::UInt(b)) => *a != (*b as f64),

            // Different types are not equal (so they are not-equal)
            _ => true,
        };
        cel_create_bool(if result { 1 } else { 0 })
    }
}

/// Polymorphic greater-than operator for CelValue objects.
/// Implements CEL spec cross-type numeric ordering.
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_gt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if a_ptr.is_null() || b_ptr.is_null() {
            error!(log, "Cannot compare null values";
                "function" => "cel_value_gt");
            abort_with_error("no such overload");
        }

        let a_val = &*a_ptr;
        let b_val = &*b_ptr;

        let result = match (a_val, b_val) {
            // Same-type comparisons
            (CelValue::Int(a), CelValue::Int(b)) => a > b,
            (CelValue::UInt(a), CelValue::UInt(b)) => a > b,
            (CelValue::Double(a), CelValue::Double(b)) => a > b,
            (CelValue::Bytes(a), CelValue::Bytes(b)) => a > b,
            (CelValue::String(a), CelValue::String(b)) => a > b,
            (CelValue::Bool(a), CelValue::Bool(b)) => a > b, // false < true in CEL
            (CelValue::Timestamp(a), CelValue::Timestamp(b)) => a > b,
            (CelValue::Duration(a), CelValue::Duration(b)) => a > b,

            // Cross-type numeric ordering
            (CelValue::Int(a), CelValue::UInt(b)) => {
                if *a < 0 {
                    false
                } else {
                    (*a as u64) > *b
                }
            }
            (CelValue::UInt(a), CelValue::Int(b)) => {
                if *b < 0 {
                    true
                } else {
                    *a > (*b as u64)
                }
            }
            (CelValue::Int(a), CelValue::Double(b)) => (*a as f64) > *b,
            (CelValue::Double(a), CelValue::Int(b)) => *a > (*b as f64),
            (CelValue::UInt(a), CelValue::Double(b)) => (*a as f64) > *b,
            (CelValue::Double(a), CelValue::UInt(b)) => *a > (*b as f64),

            _ => {
                error!(log, "Cannot compare incompatible types for greater-than";
                    "operation" => "cel_value_gt",
                    "left_type" => format!("{:?}", a_val),
                    "right_type" => format!("{:?}", b_val));
                abort_with_error("no such overload");
            }
        };
        cel_create_bool(if result { 1 } else { 0 })
    }
}

/// Polymorphic less-than operator for CelValue objects.
/// Implements CEL spec cross-type numeric ordering.
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_lt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if a_ptr.is_null() || b_ptr.is_null() {
            error!(log, "Cannot compare null values";
                "function" => "cel_value_lt");
            abort_with_error("no such overload");
        }

        let a_val = &*a_ptr;
        let b_val = &*b_ptr;

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

            _ => {
                error!(log, "Cannot compare incompatible types for less-than";
                    "operation" => "cel_value_lt",
                    "left_type" => format!("{:?}", a_val),
                    "right_type" => format!("{:?}", b_val));
                abort_with_error("no such overload");
            }
        };
        cel_create_bool(if result { 1 } else { 0 })
    }
}

/// Polymorphic greater-than-or-equal operator for CelValue objects.
/// Implements CEL spec cross-type numeric ordering.
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_gte(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if a_ptr.is_null() || b_ptr.is_null() {
            error!(log, "Cannot compare null values";
                "function" => "cel_value_gte");
            abort_with_error("no such overload");
        }

        let a_val = &*a_ptr;
        let b_val = &*b_ptr;

        let result = match (a_val, b_val) {
            // Same-type comparisons
            (CelValue::Int(a), CelValue::Int(b)) => a >= b,
            (CelValue::UInt(a), CelValue::UInt(b)) => a >= b,
            (CelValue::Double(a), CelValue::Double(b)) => a >= b,
            (CelValue::Bytes(a), CelValue::Bytes(b)) => a >= b,
            (CelValue::String(a), CelValue::String(b)) => a >= b,
            (CelValue::Bool(a), CelValue::Bool(b)) => a >= b, // false < true in CEL
            (CelValue::Timestamp(a), CelValue::Timestamp(b)) => a >= b,
            (CelValue::Duration(a), CelValue::Duration(b)) => a >= b,

            // Cross-type numeric ordering
            (CelValue::Int(a), CelValue::UInt(b)) => {
                if *a < 0 {
                    false
                } else {
                    (*a as u64) >= *b
                }
            }
            (CelValue::UInt(a), CelValue::Int(b)) => {
                if *b < 0 {
                    true
                } else {
                    *a >= (*b as u64)
                }
            }
            (CelValue::Int(a), CelValue::Double(b)) => (*a as f64) >= *b,
            (CelValue::Double(a), CelValue::Int(b)) => *a >= (*b as f64),
            (CelValue::UInt(a), CelValue::Double(b)) => (*a as f64) >= *b,
            (CelValue::Double(a), CelValue::UInt(b)) => *a >= (*b as f64),

            _ => {
                error!(log, "Cannot compare incompatible types for greater-than-or-equal";
                    "operation" => "cel_value_gte",
                    "left_type" => format!("{:?}", a_val),
                    "right_type" => format!("{:?}", b_val));
                abort_with_error("no such overload");
            }
        };
        cel_create_bool(if result { 1 } else { 0 })
    }
}

/// Polymorphic less-than-or-equal operator for CelValue objects.
/// Implements CEL spec cross-type numeric ordering.
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_lte(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if a_ptr.is_null() || b_ptr.is_null() {
            error!(log, "Cannot compare null values";
                "function" => "cel_value_lte");
            abort_with_error("no such overload");
        }

        let a_val = &*a_ptr;
        let b_val = &*b_ptr;

        let result = match (a_val, b_val) {
            // Same-type comparisons
            (CelValue::Int(a), CelValue::Int(b)) => a <= b,
            (CelValue::UInt(a), CelValue::UInt(b)) => a <= b,
            (CelValue::Double(a), CelValue::Double(b)) => a <= b,
            (CelValue::Bytes(a), CelValue::Bytes(b)) => a <= b,
            (CelValue::String(a), CelValue::String(b)) => a <= b,
            (CelValue::Bool(a), CelValue::Bool(b)) => a <= b, // false < true in CEL
            (CelValue::Timestamp(a), CelValue::Timestamp(b)) => a <= b,
            (CelValue::Duration(a), CelValue::Duration(b)) => a <= b,

            // Cross-type numeric ordering
            (CelValue::Int(a), CelValue::UInt(b)) => {
                if *a < 0 {
                    true
                } else {
                    (*a as u64) <= *b
                }
            }
            (CelValue::UInt(a), CelValue::Int(b)) => {
                if *b < 0 {
                    false
                } else {
                    *a <= (*b as u64)
                }
            }
            (CelValue::Int(a), CelValue::Double(b)) => (*a as f64) <= *b,
            (CelValue::Double(a), CelValue::Int(b)) => *a <= (*b as f64),
            (CelValue::UInt(a), CelValue::Double(b)) => (*a as f64) <= *b,
            (CelValue::Double(a), CelValue::UInt(b)) => *a <= (*b as f64),

            _ => {
                error!(log, "Cannot compare incompatible types for less-than-or-equal";
                    "operation" => "cel_value_lte",
                    "left_type" => format!("{:?}", a_val),
                    "right_type" => format!("{:?}", b_val));
                abort_with_error("no such overload");
            }
        };
        cel_create_bool(if result { 1 } else { 0 })
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
///
/// # Arguments
/// - `ptr`: Pointer to the value
///
/// # Returns
/// The size as i64
///
/// # Panics
/// - If the pointer is null
/// - If the type doesn't support size operation
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_size(ptr: *mut CelValue) -> i64 {
    let log = crate::logging::get_logger();

    unsafe {
        if ptr.is_null() {
            error!(log, "Cannot get size of null value";
                "function" => "cel_value_size");
            abort_with_error("no such overload");
        }

        let value = &*ptr;

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
}

/// Polymorphic negation operator for CelValue objects.
/// Performs unary negation:
/// - Int: arithmetic negation
/// - Double: arithmetic negation
/// - Duration: temporal negation
///
/// # Safety
/// - `ptr` must be a valid, non-null CelValue pointer
///
/// # Arguments
/// - `ptr`: Pointer to the operand
///
/// # Returns
/// Pointer to a new heap-allocated CelValue containing the negated result
///
/// # Panics
/// - If the pointer is null
/// - If the type doesn't support negation
/// - On integer overflow (negating i64::MIN)
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_negate(ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if ptr.is_null() {
            error!(log, "Cannot negate null value";
                "function" => "cel_value_negate");
            abort_with_error("no such overload");
        }

        let value = &*ptr;

        match value {
            CelValue::Int(i) => {
                debug!(log, "Performing Int negation"; "value" => *i);
                match i.checked_neg() {
                    Some(result) => cel_create_int(result),
                    None => abort_with_error("return error for overflow"),
                }
            }
            CelValue::Double(d) => {
                debug!(log, "Performing Double negation"; "value" => *d);
                cel_create_double(-d)
            }
            CelValue::Duration(_) => {
                debug!(log, "Performing Duration negation");
                temporal::cel_duration_negate(ptr)
            }
            other => {
                error!(log, "Negation not supported for this type";
                    "function" => "cel_value_negate",
                    "type" => format!("{:?}", other));
                abort_with_error("no such overload")
            }
        }
    }
}

/// Index operator for arrays and maps.
///
/// # Parameters
/// - `container_ptr`: Pointer to a CelValue (must be an Array or Object)
/// - `index_ptr`: Pointer to a CelValue to use as index (Int/UInt/Double for arrays, String for maps)
///
/// # Returns
/// - Pointer to a new CelValue containing the element at the given index
///
/// # Panics
/// - If either pointer is null
/// - If the container is not an Array or Object
/// - If the index type doesn't match the container type
/// - If the index is out of bounds (for arrays)
/// - If the key doesn't exist (for maps)
/// - If Double index is not a whole number
///
/// # Safety
/// - Both pointers must be valid CelValue pointers
#[unsafe(no_mangle)]
pub extern "C" fn cel_value_index(
    container_ptr: *mut CelValue,
    index_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    unsafe {
        if container_ptr.is_null() {
            error!(log, "Cannot index null container";
                "function" => "cel_value_index");
            abort_with_error("no such overload");
        }
        if index_ptr.is_null() {
            error!(log, "Cannot use null index";
                "function" => "cel_value_index");
            abort_with_error("no such overload");
        }

        let container = &*container_ptr;
        let index = &*index_ptr;

        match (container, index) {
            // Array indexing with Int
            (CelValue::Array(_), CelValue::Int(idx)) => {
                debug!(log, "Indexing array with Int"; "index" => *idx);
                array::cel_array_get(container_ptr, *idx as i32)
            }
            // Array indexing with UInt (convert to Int)
            (CelValue::Array(_), CelValue::UInt(idx)) => {
                debug!(log, "Indexing array with UInt"; "index" => *idx);
                let idx_i64: i64 = match (*idx).try_into() {
                    Ok(v) => v,
                    Err(_) => {
                        error!(log, "UInt index too large to convert to Int";
                            "function" => "cel_value_index",
                            "index" => *idx);
                        abort_with_error("no such overload");
                    }
                };
                array::cel_array_get(container_ptr, idx_i64 as i32)
            }
            // Array indexing with Double (convert to Int)
            (CelValue::Array(_), CelValue::Double(idx)) => {
                debug!(log, "Indexing array with Double"; "index" => *idx);
                // Check if the double is a whole number
                if idx.fract() != 0.0 {
                    error!(log, "Array index must be a whole number";
                        "function" => "cel_value_index",
                        "index" => *idx);
                    abort_with_error("no such overload");
                }
                // Convert to i64
                let idx_i64 = *idx as i64;
                array::cel_array_get(container_ptr, idx_i64 as i32)
            }
            // Map indexing with valid key types (bool, int, uint, string)
            (CelValue::Object(map), key) => {
                use crate::types::CelMapKey;
                match CelMapKey::from_cel_value(key) {
                    Some(map_key) => {
                        debug!(log, "Indexing map"; "key" => map_key.to_string_key());
                        match map.get(&map_key) {
                            Some(value) => Box::into_raw(Box::new(value.clone())),
                            None => crate::error::abort_with_error("no such key"),
                        }
                    }
                    None => {
                        error!(log, "Map key must be bool, int, uint, or string";
                            "function" => "cel_value_index",
                            "key_type" => format!("{:?}", key));
                        abort_with_error("no such overload");
                    }
                }
            }
            // Type mismatches
            (CelValue::Array(_), _) => {
                error!(log, "Array index must be Int, UInt, or Double";
                    "function" => "cel_value_index",
                    "index_type" => format!("{:?}", index));
                abort_with_error("no such overload");
            }
            _ => {
                error!(log, "Index operator not supported for this type";
                    "function" => "cel_value_index",
                    "container_type" => format!("{:?}", container),
                    "index_type" => format!("{:?}", index));
                abort_with_error("no such overload");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deserialization::cel_free_value;
    use rstest::rstest;

    #[test]
    fn test_create_int() {
        let ptr = cel_create_int(42);
        unsafe {
            assert_eq!(*ptr, CelValue::Int(42));
            // Clean up
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_bool_true() {
        let ptr = cel_create_bool(1);
        unsafe {
            assert_eq!(*ptr, CelValue::Bool(true));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_bool_false() {
        let ptr = cel_create_bool(0);
        unsafe {
            assert_eq!(*ptr, CelValue::Bool(false));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_bool_nonzero() {
        let ptr = cel_create_bool(42);
        unsafe {
            assert_eq!(*ptr, CelValue::Bool(true));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_extract_int() {
        let ptr = cel_create_int(123);
        let value = extract_int(ptr);
        assert_eq!(value, 123);
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_extract_bool_true() {
        let ptr = cel_create_bool(1);
        let value = extract_bool(ptr);
        assert_eq!(value, true);
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_extract_bool_false() {
        let ptr = cel_create_bool(0);
        let value = extract_bool(ptr);
        assert_eq!(value, false);
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_double() {
        let ptr = cel_create_double(3.14);
        unsafe {
            assert_eq!(*ptr, CelValue::Double(3.14));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_double_negative() {
        let ptr = cel_create_double(-2.5);
        unsafe {
            assert_eq!(*ptr, CelValue::Double(-2.5));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_extract_double() {
        let ptr = cel_create_double(123.456);
        let value = extract_double(ptr);
        assert_eq!(value, 123.456);
        unsafe {
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

            // Clean up
            cel_free_value(a_ptr);
            cel_free_value(b_ptr);
            cel_free_value(result_ptr);
        }
    }

    // Uint creation and extraction tests

    #[test]
    fn test_create_uint() {
        let ptr = cel_create_uint(123);
        unsafe {
            assert_eq!(*ptr, CelValue::UInt(123));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_create_uint_max() {
        let ptr = cel_create_uint(u64::MAX);
        unsafe {
            assert_eq!(*ptr, CelValue::UInt(u64::MAX));
            let _ = Box::from_raw(ptr);
        }
    }

    #[test]
    fn test_extract_uint() {
        let ptr = cel_create_uint(12345);
        let value = extract_uint(ptr);
        assert_eq!(value, 12345);
        unsafe {
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

            // Clean up
            cel_free_value(a_ptr);
            cel_free_value(b_ptr);
            cel_free_value(result_ptr);
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

            // Clean up
            cel_free_value(a_ptr);
            cel_free_value(b_ptr);
            cel_free_value(result_ptr);
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

            // Clean up
            cel_free_value(a_ptr);
            cel_free_value(b_ptr);
            cel_free_value(result_ptr);
        }
    }
}
