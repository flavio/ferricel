//! Temporal (Timestamp and Duration) arithmetic and accessor operations.
//!
//! This module implements all CEL specification timestamp and duration operations:
//! - Timestamp arithmetic (addition/subtraction with durations)
//! - Duration arithmetic (addition/subtraction/negation)
//! - Timestamp accessors (getFullYear, getMonth, etc.)
//! - Overflow checking for valid timestamp range

use crate::error::{abort_with_error, null_to_unbound};
use crate::helpers::cel_create_duration;
use crate::types::CelValue;
use chrono::{DateTime, Datelike, FixedOffset, Timelike, Utc};

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
// Pure-Rust inner functions (no raw pointers) — called by consuming wrappers
// in helpers.rs
// ============================================================================

/// timestamp + duration = timestamp (pure Rust, no pointers)
pub(crate) fn timestamp_add_duration_inner(
    dt: DateTime<FixedOffset>,
    duration: chrono::Duration,
) -> CelValue {
    let result = dt
        .checked_add_signed(duration)
        .expect("timestamp overflow in addition");
    validate_datetime(&result);
    CelValue::Timestamp(result)
}

/// timestamp - duration = timestamp (pure Rust, no pointers)
pub(crate) fn timestamp_sub_duration_inner(
    dt: DateTime<FixedOffset>,
    duration: chrono::Duration,
) -> CelValue {
    let result = dt
        .checked_sub_signed(duration)
        .expect("timestamp underflow in subtraction");
    validate_datetime(&result);
    CelValue::Timestamp(result)
}

/// timestamp - timestamp = duration (pure Rust, no pointers)
pub(crate) fn timestamp_diff_inner(
    dt1: DateTime<FixedOffset>,
    dt2: DateTime<FixedOffset>,
) -> CelValue {
    let duration = dt1.signed_duration_since(dt2);
    let seconds = duration.num_seconds();
    let nanos = duration.subsec_nanos();
    validate_duration(seconds, nanos);
    CelValue::Duration(duration)
}

/// duration + duration = duration (pure Rust, no pointers)
pub(crate) fn duration_add_inner(d1: chrono::Duration, d2: chrono::Duration) -> CelValue {
    let secs1 = d1.num_seconds();
    let nanos1 = d1.subsec_nanos();
    let secs2 = d2.num_seconds();
    let nanos2 = d2.subsec_nanos();
    let result_secs = secs1.checked_add(secs2).expect("duration overflow");
    let result_nanos = nanos1 + nanos2;
    let (final_secs, final_nanos) = normalize_duration(result_secs, result_nanos);
    validate_duration(final_secs, final_nanos);
    make_duration(final_secs, final_nanos)
}

/// duration - duration = duration (pure Rust, no pointers)
pub(crate) fn duration_sub_inner(d1: chrono::Duration, d2: chrono::Duration) -> CelValue {
    let secs1 = d1.num_seconds();
    let nanos1 = d1.subsec_nanos();
    let secs2 = d2.num_seconds();
    let nanos2 = d2.subsec_nanos();
    let result_secs = secs1.checked_sub(secs2).expect("duration underflow");
    let result_nanos = nanos1 - nanos2;
    let (final_secs, final_nanos) = normalize_duration(result_secs, result_nanos);
    validate_duration(final_secs, final_nanos);
    make_duration(final_secs, final_nanos)
}

/// -duration (pure Rust, no pointers)
pub(crate) fn duration_negate_inner(d: chrono::Duration) -> CelValue {
    let secs = d.num_seconds();
    let nanos = d.subsec_nanos();
    let neg_secs = secs.checked_neg().expect("duration negation overflow");
    let neg_nanos = -nanos;
    let (final_secs, final_nanos) = normalize_duration(neg_secs, neg_nanos);
    validate_duration(final_secs, final_nanos);
    make_duration(final_secs, final_nanos)
}

/// Construct a CelValue::Duration from (seconds, nanos).
fn make_duration(seconds: i64, nanos: i32) -> CelValue {
    CelValue::Duration(crate::chrono_helpers::parts_to_duration(
        seconds,
        nanos as i64,
    ))
}

// ============================================================================
// Timestamp + Duration = Timestamp
// ============================================================================

/// Adds a duration to a timestamp.
/// timestamp + duration = timestamp
/// Preserves the timezone of the original timestamp.
///
/// Consuming: takes ownership of both `ts_ptr` and `dur_ptr`.
///
/// # Safety
/// - Both pointers must be valid, non-null, uniquely-owned CelValue pointers
/// - First must be Timestamp, second must be Duration
///
/// # Panics
/// - If result is outside valid timestamp range
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_timestamp_add_duration(
    ts_ptr: *mut CelValue,
    dur_ptr: *mut CelValue,
) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let dur = null_to_unbound(dur_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_add_duration: expected Timestamp"),
    };
    let duration = match dur {
        CelValue::Duration(d) => d,
        _ => abort_with_error("cel_timestamp_add_duration: expected Duration"),
    };

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
/// Consuming: takes ownership of both `ts_ptr` and `dur_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_timestamp_sub_duration(
    ts_ptr: *mut CelValue,
    dur_ptr: *mut CelValue,
) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let dur = null_to_unbound(dur_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_sub_duration: expected Timestamp"),
    };
    let duration = match dur {
        CelValue::Duration(d) => d,
        _ => abort_with_error("cel_timestamp_sub_duration: expected Duration"),
    };

    let result = dt
        .checked_sub_signed(duration)
        .expect("timestamp underflow in subtraction");

    validate_datetime(&result);

    Box::into_raw(Box::new(CelValue::Timestamp(result)))
}

/// Computes the difference between two timestamps.
/// timestamp - timestamp = duration
///
/// Consuming: takes ownership of both `ts1_ptr` and `ts2_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue::Timestamp pointers.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_timestamp_diff(ts1_ptr: *mut CelValue, ts2_ptr: *mut CelValue) -> *mut CelValue {
    let ts1 = null_to_unbound(ts1_ptr);
    let ts2 = null_to_unbound(ts2_ptr);
    let dt1 = match ts1 {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_diff: expected Timestamp"),
    };
    let dt2 = match ts2 {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_diff: expected Timestamp"),
    };

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
/// Consuming: takes ownership of both `dur1_ptr` and `dur2_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue::Duration pointers.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_duration_add(dur1_ptr: *mut CelValue, dur2_ptr: *mut CelValue) -> *mut CelValue {
    let d1 = null_to_unbound(dur1_ptr);
    let d2 = null_to_unbound(dur2_ptr);
    let (secs1, nanos1) = match d1 {
        CelValue::Duration(d) => crate::chrono_helpers::duration_to_parts(&d),
        _ => abort_with_error("cel_duration_add: expected Duration"),
    };
    let (secs2, nanos2) = match d2 {
        CelValue::Duration(d) => crate::chrono_helpers::duration_to_parts(&d),
        _ => abort_with_error("cel_duration_add: expected Duration"),
    };

    let result_secs = secs1.checked_add(secs2).expect("duration overflow");
    let result_nanos = nanos1 + nanos2;

    let (final_secs, final_nanos) = normalize_duration(result_secs, result_nanos);
    validate_duration(final_secs, final_nanos);
    cel_create_duration(final_secs, final_nanos as i64)
}

/// Subtracts one duration from another.
/// duration - duration = duration
///
/// Consuming: takes ownership of both `dur1_ptr` and `dur2_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue::Duration pointers.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_duration_sub(dur1_ptr: *mut CelValue, dur2_ptr: *mut CelValue) -> *mut CelValue {
    let d1 = null_to_unbound(dur1_ptr);
    let d2 = null_to_unbound(dur2_ptr);
    let (secs1, nanos1) = match d1 {
        CelValue::Duration(d) => crate::chrono_helpers::duration_to_parts(&d),
        _ => abort_with_error("cel_duration_sub: expected Duration"),
    };
    let (secs2, nanos2) = match d2 {
        CelValue::Duration(d) => crate::chrono_helpers::duration_to_parts(&d),
        _ => abort_with_error("cel_duration_sub: expected Duration"),
    };

    let result_secs = secs1.checked_sub(secs2).expect("duration underflow");
    let result_nanos = nanos1 - nanos2;

    let (final_secs, final_nanos) = normalize_duration(result_secs, result_nanos);
    validate_duration(final_secs, final_nanos);
    cel_create_duration(final_secs, final_nanos as i64)
}

/// Negates a duration.
/// -duration = duration
///
/// Consuming: takes ownership of `dur_ptr`.
///
/// # Safety
/// The pointer must be a valid, non-null, uniquely-owned CelValue::Duration pointer.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn cel_duration_negate(dur_ptr: *mut CelValue) -> *mut CelValue {
    let d = null_to_unbound(dur_ptr);
    let (secs, nanos) = match d {
        CelValue::Duration(d) => crate::chrono_helpers::duration_to_parts(&d),
        _ => abort_with_error("cel_duration_negate: expected Duration"),
    };

    let neg_secs = secs.checked_neg().expect("duration negation overflow");
    let neg_nanos = -nanos;

    let (final_secs, final_nanos) = normalize_duration(neg_secs, neg_nanos);
    validate_duration(final_secs, final_nanos);
    cel_create_duration(final_secs, final_nanos as i64)
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
/// Consuming: takes ownership of `ts_ptr`.
///
/// # Safety
/// The pointer must be a valid, non-null, uniquely-owned CelValue::Timestamp pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_full_year(ts_ptr: *mut CelValue) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_full_year: expected Timestamp"),
    };
    let utc_dt = dt.with_timezone(&Utc);
    Box::into_raw(Box::new(CelValue::Int(utc_dt.year() as i64)))
}

/// timestamp.getFullYear(timezone) -> int
/// Returns the year in the specified timezone
///
/// Consuming: takes ownership of both `ts_ptr` and `tz_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_full_year_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let tz_val = null_to_unbound(tz_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_full_year_tz: expected Timestamp"),
    };
    let tz_str = match tz_val {
        CelValue::String(s) => s,
        _ => abort_with_error("cel_timestamp_get_full_year_tz: expected String timezone"),
    };

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let year = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).year(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).year(),
    };

    Box::into_raw(Box::new(CelValue::Int(year as i64)))
}

/// timestamp.getMonth() -> int
/// Returns the month (0-based: 0=January, 11=December) in UTC
///
/// Consuming: takes ownership of `ts_ptr`.
///
/// # Safety
/// The pointer must be a valid, non-null, uniquely-owned CelValue::Timestamp pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_month(ts_ptr: *mut CelValue) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_month: expected Timestamp"),
    };
    let utc_dt = dt.with_timezone(&Utc);
    Box::into_raw(Box::new(CelValue::Int((utc_dt.month() - 1) as i64))) // chrono returns 1-12, convert to 0-11
}

/// timestamp.getMonth(timezone) -> int
/// Returns the month (0-based) in the specified timezone
///
/// Consuming: takes ownership of both `ts_ptr` and `tz_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_month_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let tz_val = null_to_unbound(tz_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_month_tz: expected Timestamp"),
    };
    let tz_str = match tz_val {
        CelValue::String(s) => s,
        _ => abort_with_error("cel_timestamp_get_month_tz: expected String timezone"),
    };

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let month = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).month(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).month(),
    };

    Box::into_raw(Box::new(CelValue::Int((month - 1) as i64))) // Convert to 0-based
}

/// timestamp.getDate() -> int
/// Returns the day of month (1-based: 1-31) in UTC
///
/// Consuming: takes ownership of `ts_ptr`.
///
/// # Safety
/// The pointer must be a valid, non-null, uniquely-owned CelValue::Timestamp pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_date(ts_ptr: *mut CelValue) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_date: expected Timestamp"),
    };
    let utc_dt = dt.with_timezone(&Utc);
    Box::into_raw(Box::new(CelValue::Int(utc_dt.day() as i64)))
}

/// timestamp.getDate(timezone) -> int
/// Returns the day of month in the specified timezone
///
/// Consuming: takes ownership of both `ts_ptr` and `tz_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_date_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let tz_val = null_to_unbound(tz_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_date_tz: expected Timestamp"),
    };
    let tz_str = match tz_val {
        CelValue::String(s) => s,
        _ => abort_with_error("cel_timestamp_get_date_tz: expected String timezone"),
    };

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let day = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).day(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).day(),
    };

    Box::into_raw(Box::new(CelValue::Int(day as i64)))
}

/// timestamp.getDayOfMonth() -> int
/// Returns the day of month (0-based: 0-30) in UTC
///
/// Consuming: takes ownership of `ts_ptr`.
///
/// # Safety
/// The pointer must be a valid, non-null, uniquely-owned CelValue::Timestamp pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_month(ts_ptr: *mut CelValue) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_day_of_month: expected Timestamp"),
    };
    let utc_dt = dt.with_timezone(&Utc);
    Box::into_raw(Box::new(CelValue::Int((utc_dt.day() - 1) as i64))) // Convert to 0-based
}

/// timestamp.getDayOfMonth(timezone) -> int
/// Returns the day of month (0-based) in the specified timezone
///
/// Consuming: takes ownership of both `ts_ptr` and `tz_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_month_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let tz_val = null_to_unbound(tz_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_day_of_month_tz: expected Timestamp"),
    };
    let tz_str = match tz_val {
        CelValue::String(s) => s,
        _ => abort_with_error("cel_timestamp_get_day_of_month_tz: expected String timezone"),
    };

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let day = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).day(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).day(),
    };

    Box::into_raw(Box::new(CelValue::Int((day - 1) as i64))) // Convert to 0-based
}

/// timestamp.getDayOfWeek() -> int
/// Returns day of week (0=Sunday, 1=Monday, ..., 6=Saturday) in UTC
///
/// Consuming: takes ownership of `ts_ptr`.
///
/// # Safety
/// The pointer must be a valid, non-null, uniquely-owned CelValue::Timestamp pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_week(ts_ptr: *mut CelValue) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_day_of_week: expected Timestamp"),
    };
    let utc_dt = dt.with_timezone(&Utc);
    let dow = utc_dt.weekday().num_days_from_sunday();
    Box::into_raw(Box::new(CelValue::Int(dow as i64)))
}

/// timestamp.getDayOfWeek(timezone) -> int
/// Returns day of week in the specified timezone
///
/// Consuming: takes ownership of both `ts_ptr` and `tz_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_week_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let tz_val = null_to_unbound(tz_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_day_of_week_tz: expected Timestamp"),
    };
    let tz_str = match tz_val {
        CelValue::String(s) => s,
        _ => abort_with_error("cel_timestamp_get_day_of_week_tz: expected String timezone"),
    };

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

    Box::into_raw(Box::new(CelValue::Int(dow as i64)))
}

/// timestamp.getDayOfYear() -> int
/// Returns day of year (0-based: 0-365) in UTC
///
/// Consuming: takes ownership of `ts_ptr`.
///
/// # Safety
/// The pointer must be a valid, non-null, uniquely-owned CelValue::Timestamp pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_year(ts_ptr: *mut CelValue) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_day_of_year: expected Timestamp"),
    };
    let utc_dt = dt.with_timezone(&Utc);
    let doy = utc_dt.ordinal() - 1; // chrono returns 1-366, convert to 0-365
    Box::into_raw(Box::new(CelValue::Int(doy as i64)))
}

/// timestamp.getDayOfYear(timezone) -> int
/// Returns day of year (0-based) in the specified timezone
///
/// Consuming: takes ownership of both `ts_ptr` and `tz_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_year_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let tz_val = null_to_unbound(tz_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_day_of_year_tz: expected Timestamp"),
    };
    let tz_str = match tz_val {
        CelValue::String(s) => s,
        _ => abort_with_error("cel_timestamp_get_day_of_year_tz: expected String timezone"),
    };

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let doy = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).ordinal(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).ordinal(),
    };

    Box::into_raw(Box::new(CelValue::Int((doy - 1) as i64))) // Convert to 0-based
}

/// getHours() method - works on both timestamps and durations
/// - timestamp.getHours() -> Returns hour component (0-23) in UTC
/// - duration.getHours() -> Converts total duration to hours (truncated)
///
/// Consuming: takes ownership of `value_ptr`.
///
/// # Safety
/// The pointer must be a valid, non-null, uniquely-owned CelValue pointer
/// (either Timestamp or Duration).
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_hours(value_ptr: *mut CelValue) -> *mut CelValue {
    let value = null_to_unbound(value_ptr);
    match value {
        CelValue::Timestamp(dt) => {
            let utc_dt = dt.with_timezone(&Utc);
            Box::into_raw(Box::new(CelValue::Int(utc_dt.hour() as i64)))
        }
        CelValue::Duration(d) => {
            let (secs, _nanos) = crate::chrono_helpers::duration_to_parts(&d);
            Box::into_raw(Box::new(CelValue::Int(secs / 3600)))
        }
        _ => abort_with_error("getHours() must be called on a timestamp or duration"),
    }
}

/// timestamp.getHours(timezone) -> int
/// Returns hour component in the specified timezone
///
/// Consuming: takes ownership of both `ts_ptr` and `tz_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_hours_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let tz_val = null_to_unbound(tz_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_hours_tz: expected Timestamp"),
    };
    let tz_str = match tz_val {
        CelValue::String(s) => s,
        _ => abort_with_error("cel_timestamp_get_hours_tz: expected String timezone"),
    };

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let hour = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).hour(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).hour(),
    };

    Box::into_raw(Box::new(CelValue::Int(hour as i64)))
}

/// getMinutes() method - works on both timestamps and durations
/// - timestamp.getMinutes() -> Returns minutes component (0-59) in UTC
/// - duration.getMinutes() -> Converts total duration to minutes (truncated)
///
/// Consuming: takes ownership of `value_ptr`.
///
/// # Safety
/// The pointer must be a valid, non-null, uniquely-owned CelValue pointer
/// (either Timestamp or Duration).
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_minutes(value_ptr: *mut CelValue) -> *mut CelValue {
    let value = null_to_unbound(value_ptr);
    match value {
        CelValue::Timestamp(dt) => {
            let utc_dt = dt.with_timezone(&Utc);
            Box::into_raw(Box::new(CelValue::Int(utc_dt.minute() as i64)))
        }
        CelValue::Duration(d) => {
            let (secs, _nanos) = crate::chrono_helpers::duration_to_parts(&d);
            Box::into_raw(Box::new(CelValue::Int(secs / 60)))
        }
        _ => abort_with_error("getMinutes() must be called on a timestamp or duration"),
    }
}

/// timestamp.getMinutes(timezone) -> int
/// Returns minutes component in the specified timezone
///
/// Consuming: takes ownership of both `ts_ptr` and `tz_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_minutes_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let tz_val = null_to_unbound(tz_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_minutes_tz: expected Timestamp"),
    };
    let tz_str = match tz_val {
        CelValue::String(s) => s,
        _ => abort_with_error("cel_timestamp_get_minutes_tz: expected String timezone"),
    };

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let minute = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).minute(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).minute(),
    };

    Box::into_raw(Box::new(CelValue::Int(minute as i64)))
}

/// getSeconds() method - works on both timestamps and durations
/// - timestamp.getSeconds() -> Returns seconds component (0-59) in UTC
/// - duration.getSeconds() -> Returns total seconds in the duration
///
/// Consuming: takes ownership of `value_ptr`.
///
/// # Safety
/// The pointer must be a valid, non-null, uniquely-owned CelValue pointer
/// (either Timestamp or Duration).
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_seconds(value_ptr: *mut CelValue) -> *mut CelValue {
    let value = null_to_unbound(value_ptr);
    match value {
        CelValue::Timestamp(dt) => {
            let utc_dt = dt.with_timezone(&Utc);
            Box::into_raw(Box::new(CelValue::Int(utc_dt.second() as i64)))
        }
        CelValue::Duration(d) => {
            let (secs, _nanos) = crate::chrono_helpers::duration_to_parts(&d);
            Box::into_raw(Box::new(CelValue::Int(secs)))
        }
        _ => abort_with_error("getSeconds() must be called on a timestamp or duration"),
    }
}

/// timestamp.getSeconds(timezone) -> int
/// Returns seconds component in the specified timezone
///
/// Consuming: takes ownership of both `ts_ptr` and `tz_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_seconds_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let tz_val = null_to_unbound(tz_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_seconds_tz: expected Timestamp"),
    };
    let tz_str = match tz_val {
        CelValue::String(s) => s,
        _ => abort_with_error("cel_timestamp_get_seconds_tz: expected String timezone"),
    };

    let timezone = crate::chrono_helpers::parse_timezone(&tz_str)
        .unwrap_or_else(|e| panic!("Invalid timezone '{}': {}", tz_str, e));

    let second = match timezone {
        crate::chrono_helpers::Timezone::Iana(tz) => dt.with_timezone(&tz).second(),
        crate::chrono_helpers::Timezone::Fixed(offset) => dt.with_timezone(&offset).second(),
    };

    Box::into_raw(Box::new(CelValue::Int(second as i64)))
}

/// getMilliseconds() method - works on both timestamps and durations
/// - timestamp.getMilliseconds() -> Returns milliseconds component (0-999) in UTC
/// - duration.getMilliseconds() -> Returns the millisecond component of the nanoseconds part
///
/// Also handles a Duration that arrived through the JSON bridge as a
/// CelValue::Object with __type__ == "google.protobuf.Duration".
///
/// Consuming: takes ownership of `value_ptr`.
///
/// # Safety
/// The pointer must be a valid, non-null, uniquely-owned CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_milliseconds(value_ptr: *mut CelValue) -> *mut CelValue {
    let value = null_to_unbound(value_ptr);
    match value {
        CelValue::Timestamp(dt) => {
            let utc_dt = dt.with_timezone(&Utc);
            let millis = utc_dt.timestamp_subsec_millis();
            Box::into_raw(Box::new(CelValue::Int(millis as i64)))
        }
        CelValue::Duration(d) => {
            let (_secs, nanos) = crate::chrono_helpers::duration_to_parts(&d);
            let millis = nanos as i64 / 1_000_000;
            Box::into_raw(Box::new(CelValue::Int(millis)))
        }
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
                    return Box::into_raw(Box::new(CelValue::Int(*nanos / 1_000_000)));
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
/// Consuming: takes ownership of both `ts_ptr` and `tz_ptr`.
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_milliseconds_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    let ts = null_to_unbound(ts_ptr);
    let tz_val = null_to_unbound(tz_ptr);
    let dt = match ts {
        CelValue::Timestamp(dt) => dt,
        _ => abort_with_error("cel_timestamp_get_milliseconds_tz: expected Timestamp"),
    };
    let tz_str = match tz_val {
        CelValue::String(s) => s,
        _ => abort_with_error("cel_timestamp_get_milliseconds_tz: expected String timezone"),
    };

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

    Box::into_raw(Box::new(CelValue::Int(millis as i64)))
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
            // UTC: 15:30
            let ts_ptr = cel_create_timestamp(1685287800, 0);
            let utc_result = cel_timestamp_get_hours(ts_ptr);
            let utc_hours = extract_int(utc_result);
            assert_eq!(utc_hours, 15);

            // Los Angeles: 08:30 (PDT is UTC-7)
            let ts_ptr2 = cel_create_timestamp(1685287800, 0);
            let la_tz_ptr = create_string_value("America/Los_Angeles");
            let la_result = cel_timestamp_get_hours_tz(ts_ptr2, la_tz_ptr);
            let la_hours = extract_int(la_result);
            assert_eq!(la_hours, 8);

            // +09:00: 00:30 next day
            let ts_ptr3 = cel_create_timestamp(1685287800, 0);
            let tokyo_tz_ptr = create_string_value("+09:00");
            let tokyo_result = cel_timestamp_get_hours_tz(ts_ptr3, tokyo_tz_ptr);
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
            // UTC: Sunday
            let ts_ptr = cel_create_timestamp(1685287800, 0);
            let utc_result = cel_timestamp_get_day_of_week(ts_ptr);
            let utc_dow = extract_int(utc_result);
            assert_eq!(utc_dow, 0); // Sunday

            // Same in LA timezone (still Sunday)
            let ts_ptr2 = cel_create_timestamp(1685287800, 0);
            let la_tz_ptr = create_string_value("America/Los_Angeles");
            let la_result = cel_timestamp_get_day_of_week_tz(ts_ptr2, la_tz_ptr);
            let la_dow = extract_int(la_result);
            assert_eq!(la_dow, 0); // Still Sunday
        }
    }

    #[test]
    fn test_get_date_with_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z
            // UTC: day 28
            let ts_ptr = cel_create_timestamp(1685287800, 0);
            let utc_result = cel_timestamp_get_date(ts_ptr);
            let utc_day = extract_int(utc_result);
            assert_eq!(utc_day, 28);

            // LA timezone: still day 28
            let ts_ptr2 = cel_create_timestamp(1685287800, 0);
            let la_tz_ptr = create_string_value("America/Los_Angeles");
            let la_result = cel_timestamp_get_date_tz(ts_ptr2, la_tz_ptr);
            let la_day = extract_int(la_result);
            assert_eq!(la_day, 28);

            // +09:00: day 29 (crosses midnight)
            let ts_ptr3 = cel_create_timestamp(1685287800, 0);
            let tokyo_tz_ptr = create_string_value("+09:00");
            let tokyo_result = cel_timestamp_get_date_tz(ts_ptr3, tokyo_tz_ptr);
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

            let ts_ptr2 = cel_create_timestamp(1685287845, 123_000_000);
            let millis_result = cel_timestamp_get_milliseconds(ts_ptr2);
            let millis = extract_int(millis_result);
            assert_eq!(millis, 123);

            // Same with timezone
            let ts_ptr3 = cel_create_timestamp(1685287845, 123_000_000);
            let tz_ptr = create_string_value("UTC");
            let tz_seconds_result = cel_timestamp_get_seconds_tz(ts_ptr3, tz_ptr);
            let tz_seconds = extract_int(tz_seconds_result);
            assert_eq!(tz_seconds, 45);

            let ts_ptr4 = cel_create_timestamp(1685287845, 123_000_000);
            let tz_ptr2 = create_string_value("UTC");
            let tz_millis_result = cel_timestamp_get_milliseconds_tz(ts_ptr4, tz_ptr2);
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

            let ts_ptr2 = cel_create_timestamp(1685287800, 0);
            let tz_ptr = create_string_value("America/Los_Angeles");
            let tz_result = cel_timestamp_get_day_of_year_tz(ts_ptr2, tz_ptr);
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
            let (secs, nanos) = match unsafe { &*sum_ptr } {
                CelValue::Duration(d) => crate::chrono_helpers::duration_to_parts(d),
                other => panic!("expected Duration, got {:?}", other),
            };
            assert_eq!(secs, 5400);
            assert_eq!(nanos, 800_000_000);
        }
    }
}
