//! Temporal (Timestamp and Duration) arithmetic and accessor operations.
//!
//! This module implements all CEL specification timestamp and duration operations:
//! - Timestamp arithmetic (addition/subtraction with durations)
//! - Duration arithmetic (addition/subtraction/negation)
//! - Timestamp accessors (getFullYear, getMonth, etc.)
//! - Overflow checking for valid timestamp range

use crate::error::abort_with_error;
use crate::helpers::{
    cel_create_duration, cel_create_int, extract_datetime, extract_duration,
    extract_duration_chrono, extract_string,
};
use crate::types::CelValue;
use chrono::{Datelike, Timelike, Utc};

// Timestamp range constants (CEL spec)
// Min: 0001-01-01T00:00:00Z
// Max: 9999-12-31T23:59:59.999999999Z
const MIN_TIMESTAMP_SECONDS: i64 = -62135596800; // 0001-01-01T00:00:00Z
const MAX_TIMESTAMP_SECONDS: i64 = 253402300799; // 9999-12-31T23:59:59Z

// Duration range constants (CEL spec)
// CEL restricts duration range to ensure robustness and prevent edge cases.
// While protobuf Duration allows ±315576000000s, CEL uses a more conservative
// limit that's slightly less than the maximum timestamp span (9999-12-31 to 0001-01-01).
// This prevents durations that span the entire valid timestamp range.
const MIN_DURATION_SECONDS: i64 = -315_537_897_598;
const MAX_DURATION_SECONDS: i64 = 315_537_897_598;

/// Normalizes seconds and nanoseconds so nanos is always in [0, 1e9)
/// and has the same sign as seconds (or is zero).
fn normalize_duration(mut seconds: i64, mut nanos: i32) -> (i64, i32) {
    // Handle nanos overflow/underflow
    if nanos >= 1_000_000_000 {
        let overflow_secs = nanos / 1_000_000_000;
        seconds = seconds
            .checked_add(overflow_secs as i64)
            .expect("duration overflow");
        nanos %= 1_000_000_000;
    } else if nanos <= -1_000_000_000 {
        let underflow_secs = (-nanos) / 1_000_000_000;
        seconds = seconds
            .checked_sub(underflow_secs as i64)
            .expect("duration underflow");
        nanos = -((-nanos) % 1_000_000_000);
    }

    // Normalize sign: ensure nanos has same sign as seconds (or is zero)
    if seconds > 0 && nanos < 0 {
        seconds -= 1;
        nanos += 1_000_000_000;
    } else if seconds < 0 && nanos > 0 {
        seconds += 1;
        nanos -= 1_000_000_000;
    }

    (seconds, nanos)
}

// ============================================================================
// Timestamp + Duration = Timestamp
// ============================================================================

/// Adds a duration to a timestamp.
/// timestamp + duration = timestamp
/// Preserves the timezone of the original timestamp.
///
/// # Safety
/// - Both pointers must be valid, non-null CelValue pointers
/// - First must be Timestamp, second must be Duration
///
/// # Panics
/// - If result is outside valid timestamp range
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_timestamp_add_duration(
    ts_ptr: *mut CelValue,
    dur_ptr: *mut CelValue,
) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let duration = extract_duration_chrono(dur_ptr);

    // Chrono addition preserves timezone
    let result = dt
        .checked_add_signed(duration)
        .expect("timestamp overflow in addition");

    validate_datetime(&result);

    Box::into_raw(Box::new(CelValue::Timestamp(result)))
}

/// Subtracts a duration from a timestamp.
/// timestamp - duration = timestamp
/// Preserves the timezone of the original timestamp.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - `ts_ptr` points to an initialized CelValue::Timestamp instance
/// - `dur_ptr` points to an initialized CelValue::Duration instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_timestamp_sub_duration(
    ts_ptr: *mut CelValue,
    dur_ptr: *mut CelValue,
) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let duration = extract_duration_chrono(dur_ptr);

    let result = dt
        .checked_sub_signed(duration)
        .expect("timestamp underflow in subtraction");

    validate_datetime(&result);

    Box::into_raw(Box::new(CelValue::Timestamp(result)))
}

/// Computes the difference between two timestamps.
/// timestamp - timestamp = duration
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Timestamp instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_timestamp_diff(ts1_ptr: *mut CelValue, ts2_ptr: *mut CelValue) -> *mut CelValue {
    let dt1 = extract_datetime(ts1_ptr);
    let dt2 = extract_datetime(ts2_ptr);

    // Chrono subtraction returns Duration
    let duration = dt1.signed_duration_since(dt2);

    // Validate the resulting duration is within range
    let seconds = duration.num_seconds();
    let nanos = duration.subsec_nanos();
    validate_duration(seconds, nanos);

    Box::into_raw(Box::new(CelValue::Duration(duration)))
}

// ============================================================================
// Duration Arithmetic
// ============================================================================

/// Adds two durations.
/// duration + duration = duration
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Duration instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_duration_add(dur1_ptr: *mut CelValue, dur2_ptr: *mut CelValue) -> *mut CelValue {
    let (secs1, nanos1) = extract_duration(dur1_ptr);
    let (secs2, nanos2) = extract_duration(dur2_ptr);

    let result_secs = secs1.checked_add(secs2).expect("duration overflow");
    let result_nanos = nanos1 + nanos2;

    let (final_secs, final_nanos) = normalize_duration(result_secs, result_nanos);
    validate_duration(final_secs, final_nanos);
    cel_create_duration(final_secs, final_nanos as i64)
}

/// Subtracts one duration from another.
/// duration - duration = duration
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue::Duration instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_duration_sub(dur1_ptr: *mut CelValue, dur2_ptr: *mut CelValue) -> *mut CelValue {
    let (secs1, nanos1) = extract_duration(dur1_ptr);
    let (secs2, nanos2) = extract_duration(dur2_ptr);

    let result_secs = secs1.checked_sub(secs2).expect("duration underflow");
    let result_nanos = nanos1 - nanos2;

    let (final_secs, final_nanos) = normalize_duration(result_secs, result_nanos);
    validate_duration(final_secs, final_nanos);
    cel_create_duration(final_secs, final_nanos as i64)
}

/// Negates a duration.
/// -duration = duration
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned
/// - The pointer points to an initialized CelValue::Duration instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_duration_negate(dur_ptr: *mut CelValue) -> *mut CelValue {
    let (secs, nanos) = extract_duration(dur_ptr);

    let neg_secs = secs.checked_neg().expect("duration negation overflow");
    let neg_nanos = -nanos;

    let (final_secs, final_nanos) = normalize_duration(neg_secs, neg_nanos);
    validate_duration(final_secs, final_nanos);
    cel_create_duration(final_secs, final_nanos as i64)
}

// ============================================================================
// Duration Converter Methods
// ============================================================================
// These convert the entire duration to a single unit (truncated to integer).
// Unlike timestamp accessors which extract components, these return total units.

/// duration.getHours() -> int
/// Converts the entire duration to hours (truncated).
/// Example: duration('10000s').getHours() returns 2 (not 2.77...)
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned
/// - The pointer points to an initialized CelValue::Duration instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_duration_get_hours(dur_ptr: *mut CelValue) -> *mut CelValue {
    let (secs, _nanos) = extract_duration(dur_ptr);
    // Convert seconds to hours, truncating fractional part
    let hours = secs / 3600;
    cel_create_int(hours)
}

/// duration.getMinutes() -> int
/// Converts the entire duration to minutes (truncated).
/// Example: duration('3730s').getMinutes() returns 62 (not 62.16...)
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned
/// - The pointer points to an initialized CelValue::Duration instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_duration_get_minutes(dur_ptr: *mut CelValue) -> *mut CelValue {
    let (secs, _nanos) = extract_duration(dur_ptr);
    // Convert seconds to minutes, truncating fractional part
    let minutes = secs / 60;
    cel_create_int(minutes)
}

/// duration.getSeconds() -> int
/// Returns the total seconds in the duration.
/// Example: duration('3730s').getSeconds() returns 3730
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned
/// - The pointer points to an initialized CelValue::Duration instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_duration_get_seconds(dur_ptr: *mut CelValue) -> *mut CelValue {
    let (secs, _nanos) = extract_duration(dur_ptr);
    cel_create_int(secs)
}

/// duration.getMilliseconds() -> int
/// Returns the milliseconds component of the nanoseconds part of the duration.
/// Per CEL spec this is NOT the total duration converted to milliseconds —
/// it is the nanoseconds sub-second component expressed in whole milliseconds.
/// Example: duration('123.321456789s').getMilliseconds() returns 321
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned
/// - The pointer points to an initialized CelValue::Duration instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_duration_get_milliseconds(dur_ptr: *mut CelValue) -> *mut CelValue {
    let (_secs, nanos) = extract_duration(dur_ptr);
    // Return the millisecond component of the nanoseconds field only.
    let millis = nanos as i64 / 1_000_000;
    cel_create_int(millis)
}

/// Validates that a DateTime is within the valid CEL range.
fn validate_datetime(dt: &chrono::DateTime<chrono::FixedOffset>) {
    let seconds = dt.timestamp();
    if !(MIN_TIMESTAMP_SECONDS..=MAX_TIMESTAMP_SECONDS).contains(&seconds) {
        panic!(
            "timestamp out of valid range (0001-01-01 to 9999-12-31): {} seconds",
            seconds
        );
    }
}

/// Validates that a duration is within the valid CEL range.
/// Valid range: slightly less than the maximum timestamp span (±315537897598 seconds)
fn validate_duration(seconds: i64, _nanos: i32) {
    if !(MIN_DURATION_SECONDS..=MAX_DURATION_SECONDS).contains(&seconds) {
        panic!(
            "duration out of valid range (±{} seconds): {} seconds",
            MAX_DURATION_SECONDS, seconds
        );
    }
}

// ============================================================================
// Timestamp Accessor Methods
// ============================================================================
// Per CEL spec, all accessors support optional timezone parameter:
// - No parameter: Returns value in UTC (spec default)
// - With timezone: Returns value in specified timezone
// ============================================================================

/// timestamp.getFullYear() -> int
/// Returns the year in UTC (per CEL spec default)
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned
/// - The pointer points to an initialized CelValue::Timestamp instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_full_year(ts_ptr: *mut CelValue) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let utc_dt = dt.with_timezone(&Utc);
    cel_create_int(utc_dt.year() as i64)
}

/// timestamp.getFullYear(timezone) -> int
/// Returns the year in the specified timezone
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - `ts_ptr` points to an initialized CelValue::Timestamp instance
/// - `tz_ptr` points to an initialized CelValue::String instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_full_year_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let tz_str = extract_string(tz_ptr);

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let year = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).year(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).year(),
    };

    cel_create_int(year as i64)
}

/// timestamp.getMonth() -> int
/// Returns the month (0-based: 0=January, 11=December) in UTC
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned
/// - The pointer points to an initialized CelValue::Timestamp instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_month(ts_ptr: *mut CelValue) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let utc_dt = dt.with_timezone(&Utc);
    cel_create_int((utc_dt.month() - 1) as i64) // chrono returns 1-12, convert to 0-11
}

/// timestamp.getMonth(timezone) -> int
/// Returns the month (0-based) in the specified timezone
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - `ts_ptr` points to an initialized CelValue::Timestamp instance
/// - `tz_ptr` points to an initialized CelValue::String instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_month_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let tz_str = extract_string(tz_ptr);

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let month = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).month(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).month(),
    };

    cel_create_int((month - 1) as i64) // Convert to 0-based
}

/// timestamp.getDate() -> int
/// Returns the day of month (1-based: 1-31) in UTC
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned
/// - The pointer points to an initialized CelValue::Timestamp instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_date(ts_ptr: *mut CelValue) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let utc_dt = dt.with_timezone(&Utc);
    cel_create_int(utc_dt.day() as i64)
}

/// timestamp.getDate(timezone) -> int
/// Returns the day of month in the specified timezone
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - `ts_ptr` points to an initialized CelValue::Timestamp instance
/// - `tz_ptr` points to an initialized CelValue::String instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_date_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let tz_str = extract_string(tz_ptr);

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let day = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).day(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).day(),
    };

    cel_create_int(day as i64)
}

/// timestamp.getDayOfMonth() -> int
/// Returns the day of month (0-based: 0-30) in UTC
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned
/// - The pointer points to an initialized CelValue::Timestamp instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_month(ts_ptr: *mut CelValue) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let utc_dt = dt.with_timezone(&Utc);
    cel_create_int((utc_dt.day() - 1) as i64) // Convert to 0-based
}

/// timestamp.getDayOfMonth(timezone) -> int
/// Returns the day of month (0-based) in the specified timezone
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - `ts_ptr` points to an initialized CelValue::Timestamp instance
/// - `tz_ptr` points to an initialized CelValue::String instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_month_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let tz_str = extract_string(tz_ptr);

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let day = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).day(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).day(),
    };

    cel_create_int((day - 1) as i64) // Convert to 0-based
}

/// timestamp.getDayOfWeek() -> int
/// Returns day of week (0=Sunday, 1=Monday, ..., 6=Saturday) in UTC
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned
/// - The pointer points to an initialized CelValue::Timestamp instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_week(ts_ptr: *mut CelValue) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let utc_dt = dt.with_timezone(&Utc);
    let dow = utc_dt.weekday().num_days_from_sunday();
    cel_create_int(dow as i64)
}

/// timestamp.getDayOfWeek(timezone) -> int
/// Returns day of week in the specified timezone
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - `ts_ptr` points to an initialized CelValue::Timestamp instance
/// - `tz_ptr` points to an initialized CelValue::String instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_week_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let tz_str = extract_string(tz_ptr);

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let dow = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => {
            dt.with_timezone(&tz).weekday().num_days_from_sunday()
        }
        crate::chrono_helpers::Timezone::Fixed(offset) => {
            dt.with_timezone(&offset).weekday().num_days_from_sunday()
        }
    };

    cel_create_int(dow as i64)
}

/// timestamp.getDayOfYear() -> int
/// Returns day of year (0-based: 0-365) in UTC
///
/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer. The caller must ensure:
/// - `ts_ptr` is a valid, properly aligned pointer to an initialized CelValue containing a Timestamp
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_year(ts_ptr: *mut CelValue) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let utc_dt = dt.with_timezone(&Utc);
    let doy = utc_dt.ordinal() - 1; // chrono returns 1-366, convert to 0-365
    cel_create_int(doy as i64)
}

/// timestamp.getDayOfYear(timezone) -> int
/// Returns day of year (0-based) in the specified timezone
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - `ts_ptr` is a valid, properly aligned pointer to an initialized CelValue containing a Timestamp
/// - `tz_ptr` is a valid, properly aligned pointer to an initialized CelValue containing a String (timezone name)
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_year_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let tz_str = extract_string(tz_ptr);

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let doy = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).ordinal(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).ordinal(),
    };

    cel_create_int((doy - 1) as i64) // Convert to 0-based
}

/// getHours() method - works on both timestamps and durations
/// - timestamp.getHours() -> Returns hour component (0-23) in UTC
/// - duration.getHours() -> Converts total duration to hours (truncated)
///
/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer. The caller must ensure:
/// - `value_ptr` is a valid, properly aligned pointer to an initialized CelValue containing either a Timestamp or Duration
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_hours(value_ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { &*value_ptr };
    match value {
        CelValue::Timestamp(_) => {
            let dt = extract_datetime(value_ptr);
            let utc_dt = dt.with_timezone(&Utc);
            cel_create_int(utc_dt.hour() as i64)
        }
        CelValue::Duration(_) => cel_duration_get_hours(value_ptr),
        _ => abort_with_error("getHours() must be called on a timestamp or duration"),
    }
}

/// timestamp.getHours(timezone) -> int
/// Returns hour component in the specified timezone
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - `ts_ptr` is a valid, properly aligned pointer to an initialized CelValue containing a Timestamp
/// - `tz_ptr` is a valid, properly aligned pointer to an initialized CelValue containing a String (timezone name)
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_hours_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let tz_str = extract_string(tz_ptr);

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let hour = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).hour(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).hour(),
    };

    cel_create_int(hour as i64)
}

/// getMinutes() method - works on both timestamps and durations
/// - timestamp.getMinutes() -> Returns minutes component (0-59) in UTC
/// - duration.getMinutes() -> Converts total duration to minutes (truncated)
///
/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer. The caller must ensure:
/// - `value_ptr` is a valid, properly aligned pointer to an initialized CelValue containing either a Timestamp or Duration
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_minutes(value_ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { &*value_ptr };
    match value {
        CelValue::Timestamp(_) => {
            let dt = extract_datetime(value_ptr);
            let utc_dt = dt.with_timezone(&Utc);
            cel_create_int(utc_dt.minute() as i64)
        }
        CelValue::Duration(_) => cel_duration_get_minutes(value_ptr),
        _ => abort_with_error("getMinutes() must be called on a timestamp or duration"),
    }
}

/// timestamp.getMinutes(timezone) -> int
/// Returns minutes component in the specified timezone
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - `ts_ptr` is a valid, properly aligned pointer to an initialized CelValue containing a Timestamp
/// - `tz_ptr` is a valid, properly aligned pointer to an initialized CelValue containing a String (timezone name)
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_minutes_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let tz_str = extract_string(tz_ptr);

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let minute = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).minute(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).minute(),
    };

    cel_create_int(minute as i64)
}

/// getSeconds() method - works on both timestamps and durations
/// - timestamp.getSeconds() -> Returns seconds component (0-59) in UTC
/// - duration.getSeconds() -> Returns total seconds in the duration
///
/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer. The caller must ensure:
/// - `value_ptr` is a valid, properly aligned pointer to an initialized CelValue containing either a Timestamp or Duration
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_seconds(value_ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { &*value_ptr };
    match value {
        CelValue::Timestamp(_) => {
            let dt = extract_datetime(value_ptr);
            let utc_dt = dt.with_timezone(&Utc);
            cel_create_int(utc_dt.second() as i64)
        }
        CelValue::Duration(_) => cel_duration_get_seconds(value_ptr),
        _ => abort_with_error("getSeconds() must be called on a timestamp or duration"),
    }
}

/// timestamp.getSeconds(timezone) -> int
/// Returns seconds component in the specified timezone
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - `ts_ptr` is a valid, properly aligned pointer to an initialized CelValue containing a Timestamp
/// - `tz_ptr` is a valid, properly aligned pointer to an initialized CelValue containing a String (timezone name)
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_seconds_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let tz_str = extract_string(tz_ptr);

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let second = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).second(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).second(),
    };

    cel_create_int(second as i64)
}

/// getMilliseconds() method - works on both timestamps and durations
/// - timestamp.getMilliseconds() -> Returns milliseconds component (0-999) in UTC
/// - duration.getMilliseconds() -> Returns the millisecond component of the nanoseconds part
///
/// Also handles a Duration that arrived through the JSON bridge as a
/// CelValue::Object with __type__ == "google.protobuf.Duration".
///
/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer. The caller must ensure:
/// - `value_ptr` is a valid, properly aligned pointer to an initialized CelValue containing either a Timestamp or Duration
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_milliseconds(value_ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { &*value_ptr };
    match value {
        CelValue::Timestamp(_) => {
            let dt = extract_datetime(value_ptr);
            let utc_dt = dt.with_timezone(&Utc);
            let millis = utc_dt.timestamp_subsec_millis();
            cel_create_int(millis as i64)
        }
        CelValue::Duration(_) => cel_duration_get_milliseconds(value_ptr),
        CelValue::Object(map) => {
            // Handle a google.protobuf.Duration that arrived through the JSON bridge
            // as {"__type__": "google.protobuf.Duration", "seconds": <s>, "nanos": <n>}.
            use crate::types::CelMapKey;
            let type_key = CelMapKey::String("__type__".into());
            if let Some(CelValue::String(type_name)) = map.get(&type_key)
                && type_name == "google.protobuf.Duration"
            {
                let nanos_key = CelMapKey::String("nanos".into());
                if let Some(CelValue::Int(nanos)) = map.get(&nanos_key) {
                    return cel_create_int(*nanos / 1_000_000);
                }
            }
            abort_with_error("getMilliseconds() must be called on a timestamp or duration")
        }
        _ => abort_with_error("getMilliseconds() must be called on a timestamp or duration"),
    }
}

/// timestamp.getMilliseconds(timezone) -> int
/// Returns milliseconds component in the specified timezone
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - `ts_ptr` is a valid, properly aligned pointer to an initialized CelValue containing a Timestamp
/// - `tz_ptr` is a valid, properly aligned pointer to an initialized CelValue containing a String (timezone name)
/// - The returned pointer must be freed using `cel_free` when no longer needed
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_milliseconds_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let dt = extract_datetime(ts_ptr);
    let tz_str = extract_string(tz_ptr);

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let millis = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => {
            dt.with_timezone(&tz).timestamp_subsec_millis()
        }
        crate::chrono_helpers::Timezone::Fixed(offset) => {
            dt.with_timezone(&offset).timestamp_subsec_millis()
        }
    };

    cel_create_int(millis as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::{cel_create_timestamp, extract_int};
    use crate::string::cel_create_string;

    /// Helper to create a CelValue::String from a Rust string
    fn create_string_value(s: &str) -> *mut CelValue {
        let bytes = s.as_bytes();
        unsafe { cel_create_string(bytes.as_ptr(), bytes.len()) }
    }

    #[test]
    fn test_get_full_year_utc_default() {
        unsafe {
            // 2023-05-28T15:30:00Z
            let ts_ptr = cel_create_timestamp(1685287800, 0);
            let result_ptr = cel_timestamp_get_full_year(ts_ptr);
            let year = extract_int(result_ptr);
            assert_eq!(year, 2023);
        }
    }

    #[test]
    fn test_get_full_year_with_utc_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z
            let ts_ptr = cel_create_timestamp(1685287800, 0);
            let tz_ptr = create_string_value("UTC");
            let result_ptr = cel_timestamp_get_full_year_tz(ts_ptr, tz_ptr);
            let year = extract_int(result_ptr);
            assert_eq!(year, 2023);
        }
    }

    #[test]
    fn test_get_full_year_with_iana_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z -> 2023-05-28T08:30:00 in America/Los_Angeles (PDT, UTC-7)
            let ts_ptr = cel_create_timestamp(1685287800, 0);
            let tz_ptr = create_string_value("America/Los_Angeles");
            let result_ptr = cel_timestamp_get_full_year_tz(ts_ptr, tz_ptr);
            let year = extract_int(result_ptr);
            assert_eq!(year, 2023);
        }
    }

    #[test]
    fn test_get_full_year_with_fixed_offset_positive() {
        unsafe {
            // 2023-05-28T15:30:00Z -> 2023-05-29T01:00:00 in +09:30
            let ts_ptr = cel_create_timestamp(1685287800, 0);
            let tz_ptr = create_string_value("+09:30");
            let result_ptr = cel_timestamp_get_full_year_tz(ts_ptr, tz_ptr);
            let year = extract_int(result_ptr);
            assert_eq!(year, 2023);
        }
    }

    #[test]
    fn test_get_full_year_with_fixed_offset_negative() {
        unsafe {
            // 2023-05-28T15:30:00Z -> 2023-05-28T08:00:00 in -07:30
            let ts_ptr = cel_create_timestamp(1685287800, 0);
            let tz_ptr = create_string_value("-07:30");
            let result_ptr = cel_timestamp_get_full_year_tz(ts_ptr, tz_ptr);
            let year = extract_int(result_ptr);
            assert_eq!(year, 2023);
        }
    }

    #[test]
    fn test_get_month_utc_default() {
        unsafe {
            // 2023-05-28T15:30:00Z (May is month 4 in CEL, 0-indexed)
            let ts_ptr = cel_create_timestamp(1685287800, 0);
            let result_ptr = cel_timestamp_get_month(ts_ptr);
            let month = extract_int(result_ptr);
            assert_eq!(month, 4); // CEL uses 0-based months
        }
    }

    #[test]
    fn test_get_month_with_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z -> 2023-05-28T08:30:00 in America/Los_Angeles
            let ts_ptr = cel_create_timestamp(1685287800, 0);
            let tz_ptr = create_string_value("America/Los_Angeles");
            let result_ptr = cel_timestamp_get_month_tz(ts_ptr, tz_ptr);
            let month = extract_int(result_ptr);
            assert_eq!(month, 4); // Still May
        }
    }

    #[test]
    fn test_get_hours_utc_vs_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z
            let ts_ptr = cel_create_timestamp(1685287800, 0);

            // UTC: 15:30
            let utc_result = cel_timestamp_get_hours(ts_ptr);
            let utc_hours = extract_int(utc_result);
            assert_eq!(utc_hours, 15);

            // Los Angeles: 08:30 (PDT is UTC-7)
            let la_tz_ptr = create_string_value("America/Los_Angeles");
            let la_result = cel_timestamp_get_hours_tz(ts_ptr, la_tz_ptr);
            let la_hours = extract_int(la_result);
            assert_eq!(la_hours, 8);

            // +09:00: 00:30 next day
            let tokyo_tz_ptr = create_string_value("+09:00");
            let tokyo_result = cel_timestamp_get_hours_tz(ts_ptr, tokyo_tz_ptr);
            let tokyo_hours = extract_int(tokyo_result);
            assert_eq!(tokyo_hours, 0);
        }
    }

    #[test]
    fn test_get_minutes_with_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z
            let ts_ptr = cel_create_timestamp(1685287800, 0);
            let tz_ptr = create_string_value("America/Los_Angeles");
            let result_ptr = cel_timestamp_get_minutes_tz(ts_ptr, tz_ptr);
            let minutes = extract_int(result_ptr);
            assert_eq!(minutes, 30);
        }
    }

    #[test]
    fn test_get_day_of_week_with_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z is Sunday (0)
            let ts_ptr = cel_create_timestamp(1685287800, 0);

            // UTC: Sunday
            let utc_result = cel_timestamp_get_day_of_week(ts_ptr);
            let utc_dow = extract_int(utc_result);
            assert_eq!(utc_dow, 0); // Sunday

            // Same in LA timezone (still Sunday)
            let la_tz_ptr = create_string_value("America/Los_Angeles");
            let la_result = cel_timestamp_get_day_of_week_tz(ts_ptr, la_tz_ptr);
            let la_dow = extract_int(la_result);
            assert_eq!(la_dow, 0); // Still Sunday
        }
    }

    #[test]
    fn test_get_date_with_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z
            let ts_ptr = cel_create_timestamp(1685287800, 0);

            // UTC: day 28
            let utc_result = cel_timestamp_get_date(ts_ptr);
            let utc_day = extract_int(utc_result);
            assert_eq!(utc_day, 28);

            // LA timezone: still day 28
            let la_tz_ptr = create_string_value("America/Los_Angeles");
            let la_result = cel_timestamp_get_date_tz(ts_ptr, la_tz_ptr);
            let la_day = extract_int(la_result);
            assert_eq!(la_day, 28);

            // +09:00: day 29 (crosses midnight)
            let tokyo_tz_ptr = create_string_value("+09:00");
            let tokyo_result = cel_timestamp_get_date_tz(ts_ptr, tokyo_tz_ptr);
            let tokyo_day = extract_int(tokyo_result);
            assert_eq!(tokyo_day, 29);
        }
    }

    #[test]
    fn test_get_seconds_and_milliseconds() {
        unsafe {
            // 2023-05-28T15:30:45.123Z
            let ts_ptr = cel_create_timestamp(1685287845, 123_000_000);

            let seconds_result = cel_timestamp_get_seconds(ts_ptr);
            let seconds = extract_int(seconds_result);
            assert_eq!(seconds, 45);

            let millis_result = cel_timestamp_get_milliseconds(ts_ptr);
            let millis = extract_int(millis_result);
            assert_eq!(millis, 123);

            // Same with timezone
            let tz_ptr = create_string_value("UTC");
            let tz_seconds_result = cel_timestamp_get_seconds_tz(ts_ptr, tz_ptr);
            let tz_seconds = extract_int(tz_seconds_result);
            assert_eq!(tz_seconds, 45);

            let tz_ptr2 = create_string_value("UTC");
            let tz_millis_result = cel_timestamp_get_milliseconds_tz(ts_ptr, tz_ptr2);
            let tz_millis = extract_int(tz_millis_result);
            assert_eq!(tz_millis, 123);
        }
    }

    #[test]
    fn test_get_day_of_year_with_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z (May 28 is day 147 of the year, 0-indexed = 147)
            let ts_ptr = cel_create_timestamp(1685287800, 0);

            let utc_result = cel_timestamp_get_day_of_year(ts_ptr);
            let utc_doy = extract_int(utc_result);
            assert_eq!(utc_doy, 147);

            let tz_ptr = create_string_value("America/Los_Angeles");
            let tz_result = cel_timestamp_get_day_of_year_tz(ts_ptr, tz_ptr);
            let tz_doy = extract_int(tz_result);
            assert_eq!(tz_doy, 147); // Same day
        }
    }

    // Note: We cannot test panics from extern "C" functions as they cannot unwind.
    // Invalid timezone strings will cause panics at runtime with descriptive messages.

    #[test]
    fn test_timestamp_arithmetic_preserves_timezone() {
        unsafe {
            // Create timestamp: 2023-05-28T15:30:00Z
            let ts_ptr = cel_create_timestamp(1685287800, 0);

            // Create duration: 1 hour
            let dur_ptr = cel_create_duration(3600, 0);

            // Add duration
            let new_ts_ptr = cel_timestamp_add_duration(ts_ptr, dur_ptr);

            // Check the new timestamp is 2023-05-28T16:30:00Z
            let hours_result = cel_timestamp_get_hours(new_ts_ptr);
            let hours = extract_int(hours_result);
            assert_eq!(hours, 16);
        }
    }

    #[test]
    fn test_duration_arithmetic() {
        unsafe {
            // Create durations
            let dur1_ptr = cel_create_duration(3600, 500_000_000); // 1h 0.5s
            let dur2_ptr = cel_create_duration(1800, 300_000_000); // 30m 0.3s

            // Add durations
            let sum_ptr = cel_duration_add(dur1_ptr, dur2_ptr);

            // Verify result (should be 5400s + 800_000_000ns = 1h 30m 0.8s)
            let (secs, nanos) = extract_duration(sum_ptr);
            assert_eq!(secs, 5400);
            assert_eq!(nanos, 800_000_000);
        }
    }
}
