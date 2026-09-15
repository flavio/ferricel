//! Temporal (Timestamp and Duration) arithmetic and accessor operations.
//!
//! This module implements all CEL specification timestamp and duration operations:
//! - Timestamp arithmetic (addition/subtraction with durations)
//! - Duration arithmetic (addition/subtraction/negation)
//! - Timestamp accessors (getFullYear, getMonth, etc.)
//! - Overflow checking for valid timestamp range
//!
//! Every function here returns `CelValue::Error` for input it does not accept
//! (wrong type, an unparsable timezone string, an out-of-range result),
//! instead of aborting. See the "Abort, or return an error value?" section of
//! `crate::error`.

use chrono::{DateTime, Datelike, FixedOffset, Timelike, Utc};

use crate::{
    chrono_helpers::Timezone,
    error::{CelError, CelResult, into_raw_result, no_such_overload},
    memory::read_ptr,
    types::CelValue,
};

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

/// Read a `CelValue::Timestamp`, propagating an error value and rejecting
/// any other type with `no such overload`.
fn expect_timestamp(value: CelValue) -> CelResult<DateTime<FixedOffset>> {
    match value {
        CelValue::Timestamp(dt) => Ok(dt),
        other => Err(no_such_overload(&other)),
    }
}

/// Read a `CelValue::String` as a CEL timezone, propagating an error value,
/// rejecting any other type with `no such overload`, and turning an
/// unparsable timezone string into an error instead of a panic.
fn expect_timezone(value: CelValue) -> CelResult<Timezone> {
    match value {
        CelValue::String(s) => crate::chrono_helpers::parse_timezone(&s)
            .map_err(|e| CelError::new(format!("invalid timezone {s:?}: {e}"))),
        other => Err(no_such_overload(&other)),
    }
}

/// Normalizes seconds and nanoseconds so nanos is always in [0, 1e9)
/// and has the same sign as seconds (or is zero).
fn normalize_duration(mut seconds: i64, mut nanos: i32) -> CelResult<(i64, i32)> {
    // Handle nanos overflow/underflow
    if nanos >= 1_000_000_000 {
        let overflow_secs = nanos / 1_000_000_000;
        seconds = seconds
            .checked_add(overflow_secs as i64)
            .ok_or_else(|| CelError::new("duration overflow"))?;
        nanos %= 1_000_000_000;
    } else if nanos <= -1_000_000_000 {
        let underflow_secs = (-nanos) / 1_000_000_000;
        seconds = seconds
            .checked_sub(underflow_secs as i64)
            .ok_or_else(|| CelError::new("duration overflow"))?;
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

    Ok((seconds, nanos))
}

// ============================================================================
// Pure-Rust inner functions (no raw pointers) — called by consuming wrappers
// in helpers.rs. Each returns a `CelValue`, which is `CelValue::Error` on
// overflow or out-of-range instead of panicking.
// ============================================================================

/// timestamp + duration = timestamp (pure Rust, no pointers)
pub(crate) fn timestamp_add_duration_inner(
    dt: DateTime<FixedOffset>,
    duration: chrono::Duration,
) -> CelValue {
    match dt.checked_add_signed(duration) {
        Some(result) => match validate_datetime(&result) {
            Ok(()) => CelValue::Timestamp(result),
            Err(e) => CelValue::Error(e),
        },
        None => CelValue::Error(CelError::new("timestamp overflow in addition")),
    }
}

/// timestamp - duration = timestamp (pure Rust, no pointers)
pub(crate) fn timestamp_sub_duration_inner(
    dt: DateTime<FixedOffset>,
    duration: chrono::Duration,
) -> CelValue {
    match dt.checked_sub_signed(duration) {
        Some(result) => match validate_datetime(&result) {
            Ok(()) => CelValue::Timestamp(result),
            Err(e) => CelValue::Error(e),
        },
        None => CelValue::Error(CelError::new("timestamp underflow in subtraction")),
    }
}

/// timestamp - timestamp = duration (pure Rust, no pointers)
pub(crate) fn timestamp_diff_inner(
    dt1: DateTime<FixedOffset>,
    dt2: DateTime<FixedOffset>,
) -> CelValue {
    let duration = dt1.signed_duration_since(dt2);
    let seconds = duration.num_seconds();
    let nanos = duration.subsec_nanos();
    match validate_duration(seconds, nanos) {
        Ok(()) => CelValue::Duration(duration),
        Err(e) => CelValue::Error(e),
    }
}

/// duration + duration = duration (pure Rust, no pointers)
pub(crate) fn duration_add_inner(d1: chrono::Duration, d2: chrono::Duration) -> CelValue {
    let secs1 = d1.num_seconds();
    let nanos1 = d1.subsec_nanos();
    let secs2 = d2.num_seconds();
    let nanos2 = d2.subsec_nanos();
    let Some(result_secs) = secs1.checked_add(secs2) else {
        return CelValue::Error(CelError::new("duration overflow"));
    };
    match normalize_and_validate(result_secs, nanos1 + nanos2) {
        Ok(v) => v,
        Err(e) => CelValue::Error(e),
    }
}

/// duration - duration = duration (pure Rust, no pointers)
pub(crate) fn duration_sub_inner(d1: chrono::Duration, d2: chrono::Duration) -> CelValue {
    let secs1 = d1.num_seconds();
    let nanos1 = d1.subsec_nanos();
    let secs2 = d2.num_seconds();
    let nanos2 = d2.subsec_nanos();
    let Some(result_secs) = secs1.checked_sub(secs2) else {
        return CelValue::Error(CelError::new("duration overflow"));
    };
    match normalize_and_validate(result_secs, nanos1 - nanos2) {
        Ok(v) => v,
        Err(e) => CelValue::Error(e),
    }
}

/// -duration (pure Rust, no pointers)
pub(crate) fn duration_negate_inner(d: chrono::Duration) -> CelValue {
    let secs = d.num_seconds();
    let nanos = d.subsec_nanos();
    let Some(neg_secs) = secs.checked_neg() else {
        return CelValue::Error(CelError::new("duration overflow"));
    };
    match normalize_and_validate(neg_secs, -nanos) {
        Ok(v) => v,
        Err(e) => CelValue::Error(e),
    }
}

/// Normalize `(seconds, nanos)`, validate the CEL duration range, and build
/// the `CelValue::Duration`.
fn normalize_and_validate(seconds: i64, nanos: i32) -> CelResult<CelValue> {
    let (final_secs, final_nanos) = normalize_duration(seconds, nanos)?;
    validate_duration(final_secs, final_nanos)?;
    Ok(make_duration(final_secs, final_nanos))
}

/// Construct a CelValue::Duration from (seconds, nanos).
fn make_duration(seconds: i64, nanos: i32) -> CelValue {
    CelValue::Duration(crate::chrono_helpers::parts_to_duration(
        seconds,
        nanos as i64,
    ))
}

/// Validates that a DateTime is within the valid CEL range.
fn validate_datetime(dt: &chrono::DateTime<chrono::FixedOffset>) -> CelResult<()> {
    let seconds = dt.timestamp();
    if !(MIN_TIMESTAMP_SECONDS..=MAX_TIMESTAMP_SECONDS).contains(&seconds) {
        return Err(CelError::new(format!(
            "timestamp out of valid range (0001-01-01 to 9999-12-31): {seconds} seconds"
        )));
    }
    Ok(())
}

/// Validates that a duration is within the valid CEL range.
/// Valid range: slightly less than the maximum timestamp span (±315537897598 seconds)
fn validate_duration(seconds: i64, _nanos: i32) -> CelResult<()> {
    if !(MIN_DURATION_SECONDS..=MAX_DURATION_SECONDS).contains(&seconds) {
        return Err(CelError::new(format!(
            "duration out of valid range (±{MAX_DURATION_SECONDS} seconds): {seconds} seconds"
        )));
    }
    Ok(())
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
/// The pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_full_year(ts_ptr: *mut CelValue) -> *mut CelValue {
    into_raw_result(
        expect_timestamp(unsafe { read_ptr(ts_ptr) })
            .map(|dt| CelValue::Int(dt.with_timezone(&Utc).year() as i64)),
    )
}

/// timestamp.getFullYear(timezone) -> int
/// Returns the year in the specified timezone
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_full_year_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    into_raw_result((|| {
        let dt = expect_timestamp(unsafe { read_ptr(ts_ptr) })?;
        let tz = expect_timezone(unsafe { read_ptr(tz_ptr) })?;
        let year = match tz {
            Timezone::Iana(tz) => dt.with_timezone(&tz).year(),
            Timezone::Fixed(offset) => dt.with_timezone(&offset).year(),
        };
        Ok(CelValue::Int(year as i64))
    })())
}

/// timestamp.getMonth() -> int
/// Returns the month (0-based: 0=January, 11=December) in UTC
///
/// # Safety
/// The pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_month(ts_ptr: *mut CelValue) -> *mut CelValue {
    into_raw_result(
        expect_timestamp(unsafe { read_ptr(ts_ptr) })
            .map(|dt| CelValue::Int((dt.with_timezone(&Utc).month() - 1) as i64)),
    )
}

/// timestamp.getMonth(timezone) -> int
/// Returns the month (0-based) in the specified timezone
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_month_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    into_raw_result((|| {
        let dt = expect_timestamp(unsafe { read_ptr(ts_ptr) })?;
        let tz = expect_timezone(unsafe { read_ptr(tz_ptr) })?;
        let month = match tz {
            Timezone::Iana(tz) => dt.with_timezone(&tz).month(),
            Timezone::Fixed(offset) => dt.with_timezone(&offset).month(),
        };
        Ok(CelValue::Int((month - 1) as i64)) // Convert to 0-based
    })())
}

/// timestamp.getDate() -> int
/// Returns the day of month (1-based: 1-31) in UTC
///
/// # Safety
/// The pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_date(ts_ptr: *mut CelValue) -> *mut CelValue {
    into_raw_result(
        expect_timestamp(unsafe { read_ptr(ts_ptr) })
            .map(|dt| CelValue::Int(dt.with_timezone(&Utc).day() as i64)),
    )
}

/// timestamp.getDate(timezone) -> int
/// Returns the day of month in the specified timezone
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_date_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    into_raw_result((|| {
        let dt = expect_timestamp(unsafe { read_ptr(ts_ptr) })?;
        let tz = expect_timezone(unsafe { read_ptr(tz_ptr) })?;
        let day = match tz {
            Timezone::Iana(tz) => dt.with_timezone(&tz).day(),
            Timezone::Fixed(offset) => dt.with_timezone(&offset).day(),
        };
        Ok(CelValue::Int(day as i64))
    })())
}

/// timestamp.getDayOfMonth() -> int
/// Returns the day of month (0-based: 0-30) in UTC
///
/// # Safety
/// The pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_month(ts_ptr: *mut CelValue) -> *mut CelValue {
    into_raw_result(
        expect_timestamp(unsafe { read_ptr(ts_ptr) })
            .map(|dt| CelValue::Int((dt.with_timezone(&Utc).day() - 1) as i64)),
    )
}

/// timestamp.getDayOfMonth(timezone) -> int
/// Returns the day of month (0-based) in the specified timezone
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_month_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    into_raw_result((|| {
        let dt = expect_timestamp(unsafe { read_ptr(ts_ptr) })?;
        let tz = expect_timezone(unsafe { read_ptr(tz_ptr) })?;
        let day = match tz {
            Timezone::Iana(tz) => dt.with_timezone(&tz).day(),
            Timezone::Fixed(offset) => dt.with_timezone(&offset).day(),
        };
        Ok(CelValue::Int((day - 1) as i64)) // Convert to 0-based
    })())
}

/// timestamp.getDayOfWeek() -> int
/// Returns day of week (0=Sunday, 1=Monday, ..., 6=Saturday) in UTC
///
/// # Safety
/// The pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_week(ts_ptr: *mut CelValue) -> *mut CelValue {
    into_raw_result(expect_timestamp(unsafe { read_ptr(ts_ptr) }).map(|dt| {
        let dow = dt.with_timezone(&Utc).weekday().num_days_from_sunday();
        CelValue::Int(dow as i64)
    }))
}

/// timestamp.getDayOfWeek(timezone) -> int
/// Returns day of week in the specified timezone
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_week_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    into_raw_result((|| {
        let dt = expect_timestamp(unsafe { read_ptr(ts_ptr) })?;
        let tz = expect_timezone(unsafe { read_ptr(tz_ptr) })?;
        let dow = match tz {
            Timezone::Iana(tz) => dt.with_timezone(&tz).weekday().num_days_from_sunday(),
            Timezone::Fixed(offset) => dt.with_timezone(&offset).weekday().num_days_from_sunday(),
        };
        Ok(CelValue::Int(dow as i64))
    })())
}

/// timestamp.getDayOfYear() -> int
/// Returns day of year (0-based: 0-365) in UTC
///
/// # Safety
/// The pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_year(ts_ptr: *mut CelValue) -> *mut CelValue {
    into_raw_result(expect_timestamp(unsafe { read_ptr(ts_ptr) }).map(|dt| {
        let doy = dt.with_timezone(&Utc).ordinal() - 1; // chrono returns 1-366, convert to 0-365
        CelValue::Int(doy as i64)
    }))
}

/// timestamp.getDayOfYear(timezone) -> int
/// Returns day of year (0-based) in the specified timezone
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_day_of_year_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    into_raw_result((|| {
        let dt = expect_timestamp(unsafe { read_ptr(ts_ptr) })?;
        let tz = expect_timezone(unsafe { read_ptr(tz_ptr) })?;
        let doy = match tz {
            Timezone::Iana(tz) => dt.with_timezone(&tz).ordinal(),
            Timezone::Fixed(offset) => dt.with_timezone(&offset).ordinal(),
        };
        Ok(CelValue::Int((doy - 1) as i64)) // Convert to 0-based
    })())
}

/// getHours() method - works on both timestamps and durations
/// - timestamp.getHours() -> Returns hour component (0-23) in UTC
/// - duration.getHours() -> Converts total duration to hours (truncated)
///
/// # Safety
/// The pointer must be a valid, non-null CelValue pointer
/// (either Timestamp or Duration).
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_hours(value_ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(value_ptr) };
    into_raw_result(match value {
        CelValue::Timestamp(dt) => Ok(CelValue::Int(dt.with_timezone(&Utc).hour() as i64)),
        CelValue::Duration(d) => {
            let (secs, _nanos) = crate::chrono_helpers::duration_to_parts(&d);
            Ok(CelValue::Int(secs / 3600))
        }
        other => Err(no_such_overload(&other)),
    })
}

/// timestamp.getHours(timezone) -> int
/// Returns hour component in the specified timezone
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_hours_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    into_raw_result((|| {
        let dt = expect_timestamp(unsafe { read_ptr(ts_ptr) })?;
        let tz = expect_timezone(unsafe { read_ptr(tz_ptr) })?;
        let hour = match tz {
            Timezone::Iana(tz) => dt.with_timezone(&tz).hour(),
            Timezone::Fixed(offset) => dt.with_timezone(&offset).hour(),
        };
        Ok(CelValue::Int(hour as i64))
    })())
}

/// getMinutes() method - works on both timestamps and durations
/// - timestamp.getMinutes() -> Returns minutes component (0-59) in UTC
/// - duration.getMinutes() -> Converts total duration to minutes (truncated)
///
/// # Safety
/// The pointer must be a valid, non-null CelValue pointer
/// (either Timestamp or Duration).
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_minutes(value_ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(value_ptr) };
    into_raw_result(match value {
        CelValue::Timestamp(dt) => Ok(CelValue::Int(dt.with_timezone(&Utc).minute() as i64)),
        CelValue::Duration(d) => {
            let (secs, _nanos) = crate::chrono_helpers::duration_to_parts(&d);
            Ok(CelValue::Int(secs / 60))
        }
        other => Err(no_such_overload(&other)),
    })
}

/// timestamp.getMinutes(timezone) -> int
/// Returns minutes component in the specified timezone
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_minutes_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    into_raw_result((|| {
        let dt = expect_timestamp(unsafe { read_ptr(ts_ptr) })?;
        let tz = expect_timezone(unsafe { read_ptr(tz_ptr) })?;
        let minute = match tz {
            Timezone::Iana(tz) => dt.with_timezone(&tz).minute(),
            Timezone::Fixed(offset) => dt.with_timezone(&offset).minute(),
        };
        Ok(CelValue::Int(minute as i64))
    })())
}

/// getSeconds() method - works on both timestamps and durations
/// - timestamp.getSeconds() -> Returns seconds component (0-59) in UTC
/// - duration.getSeconds() -> Returns total seconds in the duration
///
/// # Safety
/// The pointer must be a valid, non-null CelValue pointer
/// (either Timestamp or Duration).
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_seconds(value_ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(value_ptr) };
    into_raw_result(match value {
        CelValue::Timestamp(dt) => Ok(CelValue::Int(dt.with_timezone(&Utc).second() as i64)),
        CelValue::Duration(d) => {
            let (secs, _nanos) = crate::chrono_helpers::duration_to_parts(&d);
            Ok(CelValue::Int(secs))
        }
        other => Err(no_such_overload(&other)),
    })
}

/// timestamp.getSeconds(timezone) -> int
/// Returns seconds component in the specified timezone
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_seconds_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    into_raw_result((|| {
        let dt = expect_timestamp(unsafe { read_ptr(ts_ptr) })?;
        let tz = expect_timezone(unsafe { read_ptr(tz_ptr) })?;
        let second = match tz {
            Timezone::Iana(tz) => dt.with_timezone(&tz).second(),
            Timezone::Fixed(offset) => dt.with_timezone(&offset).second(),
        };
        Ok(CelValue::Int(second as i64))
    })())
}

/// getMilliseconds() method - works on both timestamps and durations
/// - timestamp.getMilliseconds() -> Returns milliseconds component (0-999) in UTC
/// - duration.getMilliseconds() -> Returns the millisecond component of the nanoseconds part
///
/// Also handles a Duration that arrived through the JSON bridge as a
/// CelValue::Object with __type__ == "google.protobuf.Duration".
///
/// # Safety
/// The pointer must be a valid, non-null CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_milliseconds(value_ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { read_ptr(value_ptr) };
    into_raw_result(match value {
        CelValue::Timestamp(dt) => {
            let millis = dt.with_timezone(&Utc).timestamp_subsec_millis();
            Ok(CelValue::Int(millis as i64))
        }
        CelValue::Duration(d) => {
            let (_secs, nanos) = crate::chrono_helpers::duration_to_parts(&d);
            Ok(CelValue::Int(nanos as i64 / 1_000_000))
        }
        CelValue::Object(ref map) => {
            // Handle a google.protobuf.Duration that arrived through the JSON bridge
            // as {"__type__": "google.protobuf.Duration", "seconds": <s>, "nanos": <n>}.
            use crate::types::CelMapKey;
            let type_key = CelMapKey::String("__type__".into());
            if let Some(CelValue::String(type_name)) = map.get(&type_key)
                && type_name == "google.protobuf.Duration"
            {
                let nanos_key = CelMapKey::String("nanos".into());
                if let Some(CelValue::Int(nanos)) = map.get(&nanos_key) {
                    return into_raw_result(Ok(CelValue::Int(*nanos / 1_000_000)));
                }
            }
            Err(no_such_overload(&value))
        }
        other => Err(no_such_overload(&other)),
    })
}

/// timestamp.getMilliseconds(timezone) -> int
/// Returns milliseconds component in the specified timezone
///
/// # Safety
/// Both pointers must be valid, non-null CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_timestamp_get_milliseconds_tz(
    ts_ptr: *mut CelValue,
    tz_ptr: *mut CelValue,
) -> *mut CelValue {
    into_raw_result((|| {
        let dt = expect_timestamp(unsafe { read_ptr(ts_ptr) })?;
        let tz = expect_timezone(unsafe { read_ptr(tz_ptr) })?;
        let millis = match tz {
            Timezone::Iana(tz) => dt.with_timezone(&tz).timestamp_subsec_millis(),
            Timezone::Fixed(offset) => dt.with_timezone(&offset).timestamp_subsec_millis(),
        };
        Ok(CelValue::Int(millis as i64))
    })())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        chrono_helpers::parts_to_duration, helpers::extract_int, string::cel_create_string,
    };

    /// Helper to create a chrono::DateTime<FixedOffset> from unix seconds + nanos
    fn make_timestamp(seconds: i64, nanos: i64) -> DateTime<FixedOffset> {
        use chrono::{TimeZone, Utc};
        Utc.timestamp_opt(seconds, nanos as u32)
            .single()
            .expect("Invalid timestamp")
            .into()
    }

    /// Helper to create a CelValue::String from a Rust string
    fn create_string_value(s: &str) -> *mut CelValue {
        let bytes = s.as_bytes();
        unsafe { cel_create_string(bytes.as_ptr(), bytes.len()) }
    }

    /// Assert that `ptr` (as returned by an accessor) is a `CelValue::Error`
    /// whose message contains `expected_substring`.
    unsafe fn assert_error_containing(ptr: *mut CelValue, expected_substring: &str) {
        let value = unsafe { Box::from_raw(ptr) };
        match *value {
            CelValue::Error(err) => assert!(
                err.message.contains(expected_substring),
                "expected {expected_substring:?} in {:?}",
                err.message
            ),
            other => panic!("expected an error value, got {other:?}"),
        }
    }

    #[test]
    fn test_get_full_year_utc_default() {
        unsafe {
            // 2023-05-28T15:30:00Z
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let result_ptr = cel_timestamp_get_full_year(ts_ptr);
            let year = extract_int(result_ptr);
            assert_eq!(year, 2023);
            // ts_ptr is consumed by read_ptr inside the accessor; only free result
            drop(Box::from_raw(result_ptr));
        }
    }

    #[test]
    fn test_get_full_year_with_utc_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let tz_ptr = create_string_value("UTC");
            let result_ptr = cel_timestamp_get_full_year_tz(ts_ptr, tz_ptr);
            let year = extract_int(result_ptr);
            assert_eq!(year, 2023);
            // ts_ptr and tz_ptr consumed by read_ptr inside accessor
            drop(Box::from_raw(result_ptr));
        }
    }

    #[test]
    fn test_get_full_year_with_iana_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z -> 2023-05-28T08:30:00 in America/Los_Angeles (PDT, UTC-7)
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let tz_ptr = create_string_value("America/Los_Angeles");
            let result_ptr = cel_timestamp_get_full_year_tz(ts_ptr, tz_ptr);
            let year = extract_int(result_ptr);
            assert_eq!(year, 2023);
            drop(Box::from_raw(result_ptr));
        }
    }

    #[test]
    fn test_get_full_year_with_fixed_offset_positive() {
        unsafe {
            // 2023-05-28T15:30:00Z -> 2023-05-29T01:00:00 in +09:30
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let tz_ptr = create_string_value("+09:30");
            let result_ptr = cel_timestamp_get_full_year_tz(ts_ptr, tz_ptr);
            let year = extract_int(result_ptr);
            assert_eq!(year, 2023);
            drop(Box::from_raw(result_ptr));
        }
    }

    #[test]
    fn test_get_full_year_with_fixed_offset_negative() {
        unsafe {
            // 2023-05-28T15:30:00Z -> 2023-05-28T08:00:00 in -07:30
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let tz_ptr = create_string_value("-07:30");
            let result_ptr = cel_timestamp_get_full_year_tz(ts_ptr, tz_ptr);
            let year = extract_int(result_ptr);
            assert_eq!(year, 2023);
            drop(Box::from_raw(result_ptr));
        }
    }

    #[test]
    fn test_get_month_utc_default() {
        unsafe {
            // 2023-05-28T15:30:00Z (May is month 4 in CEL, 0-indexed)
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let result_ptr = cel_timestamp_get_month(ts_ptr);
            let month = extract_int(result_ptr);
            assert_eq!(month, 4); // CEL uses 0-based months
            drop(Box::from_raw(result_ptr));
        }
    }

    #[test]
    fn test_get_month_with_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z -> 2023-05-28T08:30:00 in America/Los_Angeles
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let tz_ptr = create_string_value("America/Los_Angeles");
            let result_ptr = cel_timestamp_get_month_tz(ts_ptr, tz_ptr);
            let month = extract_int(result_ptr);
            assert_eq!(month, 4); // Still May
            drop(Box::from_raw(result_ptr));
        }
    }

    #[test]
    fn test_get_hours_utc_vs_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z UTC: 15:30
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let utc_result = cel_timestamp_get_hours(ts_ptr);
            let utc_hours = extract_int(utc_result);
            assert_eq!(utc_hours, 15);
            drop(Box::from_raw(utc_result));

            // Los Angeles: 08:30 (PDT is UTC-7)
            let ts_ptr2 =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let la_tz_ptr = create_string_value("America/Los_Angeles");
            let la_result = cel_timestamp_get_hours_tz(ts_ptr2, la_tz_ptr);
            let la_hours = extract_int(la_result);
            assert_eq!(la_hours, 8);
            drop(Box::from_raw(la_result));

            // +09:00: 00:30 next day
            let ts_ptr3 =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let tokyo_tz_ptr = create_string_value("+09:00");
            let tokyo_result = cel_timestamp_get_hours_tz(ts_ptr3, tokyo_tz_ptr);
            let tokyo_hours = extract_int(tokyo_result);
            assert_eq!(tokyo_hours, 0);
            drop(Box::from_raw(tokyo_result));
        }
    }

    #[test]
    fn test_get_minutes_with_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let tz_ptr = create_string_value("America/Los_Angeles");
            let result_ptr = cel_timestamp_get_minutes_tz(ts_ptr, tz_ptr);
            let minutes = extract_int(result_ptr);
            assert_eq!(minutes, 30);
            drop(Box::from_raw(result_ptr));
        }
    }

    #[test]
    fn test_get_day_of_week_with_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z is Sunday (0) in UTC
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let utc_result = cel_timestamp_get_day_of_week(ts_ptr);
            let utc_dow = extract_int(utc_result);
            assert_eq!(utc_dow, 0); // Sunday
            drop(Box::from_raw(utc_result));

            // Same in LA timezone (still Sunday)
            let ts_ptr2 =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let la_tz_ptr = create_string_value("America/Los_Angeles");
            let la_result = cel_timestamp_get_day_of_week_tz(ts_ptr2, la_tz_ptr);
            let la_dow = extract_int(la_result);
            assert_eq!(la_dow, 0); // Still Sunday
            drop(Box::from_raw(la_result));
        }
    }

    #[test]
    fn test_get_date_with_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z UTC: day 28
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let utc_result = cel_timestamp_get_date(ts_ptr);
            let utc_day = extract_int(utc_result);
            assert_eq!(utc_day, 28);
            drop(Box::from_raw(utc_result));

            // LA timezone: still day 28
            let ts_ptr2 =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let la_tz_ptr = create_string_value("America/Los_Angeles");
            let la_result = cel_timestamp_get_date_tz(ts_ptr2, la_tz_ptr);
            let la_day = extract_int(la_result);
            assert_eq!(la_day, 28);
            drop(Box::from_raw(la_result));

            // +09:00: day 29 (crosses midnight)
            let ts_ptr3 =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let tokyo_tz_ptr = create_string_value("+09:00");
            let tokyo_result = cel_timestamp_get_date_tz(ts_ptr3, tokyo_tz_ptr);
            let tokyo_day = extract_int(tokyo_result);
            assert_eq!(tokyo_day, 29);
            drop(Box::from_raw(tokyo_result));
        }
    }

    #[test]
    fn test_get_seconds_and_milliseconds() {
        unsafe {
            // 2023-05-28T15:30:45.123Z
            let ts_ptr = Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(
                1685287845,
                123_000_000,
            ))));
            let seconds_result = cel_timestamp_get_seconds(ts_ptr);
            let seconds = extract_int(seconds_result);
            assert_eq!(seconds, 45);
            drop(Box::from_raw(seconds_result));

            let ts_ptr2 = Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(
                1685287845,
                123_000_000,
            ))));
            let millis_result = cel_timestamp_get_milliseconds(ts_ptr2);
            let millis = extract_int(millis_result);
            assert_eq!(millis, 123);
            drop(Box::from_raw(millis_result));

            // Same with timezone
            let ts_ptr3 = Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(
                1685287845,
                123_000_000,
            ))));
            let tz_ptr = create_string_value("UTC");
            let tz_seconds_result = cel_timestamp_get_seconds_tz(ts_ptr3, tz_ptr);
            let tz_seconds = extract_int(tz_seconds_result);
            assert_eq!(tz_seconds, 45);
            drop(Box::from_raw(tz_seconds_result));

            let ts_ptr4 = Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(
                1685287845,
                123_000_000,
            ))));
            let tz_ptr2 = create_string_value("UTC");
            let tz_millis_result = cel_timestamp_get_milliseconds_tz(ts_ptr4, tz_ptr2);
            let tz_millis = extract_int(tz_millis_result);
            assert_eq!(tz_millis, 123);
            drop(Box::from_raw(tz_millis_result));
        }
    }

    #[test]
    fn test_get_day_of_year_with_timezone() {
        unsafe {
            // 2023-05-28T15:30:00Z (May 28 is day 147 of the year, 0-indexed = 147)
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let utc_result = cel_timestamp_get_day_of_year(ts_ptr);
            let utc_doy = extract_int(utc_result);
            assert_eq!(utc_doy, 147);
            drop(Box::from_raw(utc_result));

            let ts_ptr2 =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let tz_ptr = create_string_value("America/Los_Angeles");
            let tz_result = cel_timestamp_get_day_of_year_tz(ts_ptr2, tz_ptr);
            let tz_doy = extract_int(tz_result);
            assert_eq!(tz_doy, 147); // Same day
            drop(Box::from_raw(tz_result));
        }
    }

    /// Every accessor rejects a non-Timestamp receiver as an error value
    /// (not a panic), and propagates an incoming error unchanged.
    #[rstest]
    #[case::wrong_type(CelValue::Int(1), "no such overload")]
    #[case::propagates_error(
        CelValue::Error(CelError::new("no such key: 'x'")),
        "no such key: 'x'"
    )]
    fn get_full_year_rejects_non_timestamp(#[case] input: CelValue, #[case] expected: &str) {
        unsafe {
            let ptr = Box::into_raw(Box::new(input));
            let result = cel_timestamp_get_full_year(ptr);
            assert_error_containing(result, expected);
        }
    }

    #[test]
    fn get_full_year_tz_rejects_non_string_timezone() {
        unsafe {
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let tz_ptr = Box::into_raw(Box::new(CelValue::Int(1)));
            let result = cel_timestamp_get_full_year_tz(ts_ptr, tz_ptr);
            assert_error_containing(result, "no such overload");
        }
    }

    #[test]
    fn get_full_year_tz_rejects_unparsable_timezone() {
        unsafe {
            let ts_ptr =
                Box::into_raw(Box::new(CelValue::Timestamp(make_timestamp(1685287800, 0))));
            let tz_ptr = create_string_value("Not/A_Timezone");
            let result = cel_timestamp_get_full_year_tz(ts_ptr, tz_ptr);
            assert_error_containing(result, "invalid timezone");
        }
    }

    #[test]
    fn get_hours_rejects_non_timestamp_non_duration() {
        unsafe {
            let ptr = Box::into_raw(Box::new(CelValue::Bool(true)));
            let result = cel_timestamp_get_hours(ptr);
            assert_error_containing(result, "no such overload");
        }
    }

    #[test]
    fn get_milliseconds_rejects_object_without_duration_tag() {
        unsafe {
            let ptr = Box::into_raw(Box::new(CelValue::Object(Default::default())));
            let result = cel_timestamp_get_milliseconds(ptr);
            assert_error_containing(result, "no such overload");
        }
    }

    #[test]
    fn test_timestamp_add_duration_inner() {
        // 2023-05-28T15:30:00Z + 1 hour = 2023-05-28T16:30:00Z
        let dt = make_timestamp(1685287800, 0);
        let dur = parts_to_duration(3600, 0);
        let result = timestamp_add_duration_inner(dt, dur);
        unsafe {
            let new_ts_ptr = Box::into_raw(Box::new(result));
            let hours_ptr = cel_timestamp_get_hours(new_ts_ptr);
            // new_ts_ptr is consumed by read_ptr inside cel_timestamp_get_hours
            let hours = extract_int(hours_ptr);
            assert_eq!(hours, 16);
            drop(Box::from_raw(hours_ptr));
        }
    }

    #[test]
    fn test_timestamp_add_duration_inner_overflow_is_an_error() {
        let dt = make_timestamp(MAX_TIMESTAMP_SECONDS, 0);
        let dur = parts_to_duration(3600, 0);
        let result = timestamp_add_duration_inner(dt, dur);
        match result {
            CelValue::Error(err) => assert!(err.message.contains("out of valid range")),
            other => panic!("expected an error value, got {other:?}"),
        }
    }

    #[test]
    fn test_timestamp_sub_duration_inner() {
        // 2023-05-28T15:30:00Z - 1 hour = 2023-05-28T14:30:00Z
        let dt = make_timestamp(1685287800, 0);
        let dur = parts_to_duration(3600, 0);
        let result = timestamp_sub_duration_inner(dt, dur);
        unsafe {
            let new_ts_ptr = Box::into_raw(Box::new(result));
            let hours_ptr = cel_timestamp_get_hours(new_ts_ptr);
            // new_ts_ptr is consumed by read_ptr inside cel_timestamp_get_hours
            let hours = extract_int(hours_ptr);
            assert_eq!(hours, 14);
            drop(Box::from_raw(hours_ptr));
        }
    }

    #[test]
    fn test_timestamp_diff_inner() {
        // 2023-05-28T15:30:00Z - 2023-05-28T14:30:00Z = 3600s
        let dt1 = make_timestamp(1685287800, 0);
        let dt2 = make_timestamp(1685284200, 0);
        let result = timestamp_diff_inner(dt1, dt2);
        match result {
            CelValue::Duration(d) => assert_eq!(d.num_seconds(), 3600),
            other => panic!("expected Duration, got {:?}", other),
        }
    }

    #[test]
    fn test_duration_add_inner() {
        // 1h 0.5s + 30m 0.3s = 1h30m 0.8s (5400s + 800_000_000ns)
        let d1 = parts_to_duration(3600, 500_000_000);
        let d2 = parts_to_duration(1800, 300_000_000);
        let result = duration_add_inner(d1, d2);
        match result {
            CelValue::Duration(d) => {
                let (secs, nanos) = crate::chrono_helpers::duration_to_parts(&d);
                assert_eq!(secs, 5400);
                assert_eq!(nanos, 800_000_000);
            }
            other => panic!("expected Duration, got {:?}", other),
        }
    }

    #[test]
    fn test_duration_sub_inner() {
        // 1h - 30m = 30m (1800s)
        let d1 = parts_to_duration(3600, 0);
        let d2 = parts_to_duration(1800, 0);
        let result = duration_sub_inner(d1, d2);
        match result {
            CelValue::Duration(d) => assert_eq!(d.num_seconds(), 1800),
            other => panic!("expected Duration, got {:?}", other),
        }
    }

    #[test]
    fn test_duration_negate_inner() {
        // -1h = -3600s
        let d = parts_to_duration(3600, 0);
        let result = duration_negate_inner(d);
        match result {
            CelValue::Duration(d) => assert_eq!(d.num_seconds(), -3600),
            other => panic!("expected Duration, got {:?}", other),
        }
    }
}
