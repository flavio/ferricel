//! Chrono helper functions for duration and timestamp conversions.
//! Provides wrappers around chrono types for FFI boundary and parsing/formatting.

use chrono::{DateTime, Datelike, Duration, FixedOffset, TimeZone, Utc};
use chrono_tz::Tz;

/// Convert (seconds, nanos) to chrono::DateTime<FixedOffset>
///
/// # Parameters
/// - `seconds`: Unix timestamp seconds
/// - `nanos`: Nanoseconds component
///
/// # Returns
/// chrono::DateTime with UTC timezone (converted to FixedOffset)
///
/// # Panics
/// If timestamp is out of range for chrono
pub fn parts_to_datetime(seconds: i64, nanos: i64) -> DateTime<FixedOffset> {
    Utc.timestamp_opt(seconds, nanos as u32)
        .single()
        .expect("Invalid timestamp")
        .into()
}

/// Convert (seconds, nanos) to chrono::Duration
///
/// # Parameters
/// - `seconds`: Duration seconds
/// - `nanos`: Nanoseconds component
///
/// # Returns
/// chrono::Duration
pub fn parts_to_duration(seconds: i64, nanos: i64) -> Duration {
    Duration::seconds(seconds) + Duration::nanoseconds(nanos)
}

/// Convert chrono::Duration to (seconds, nanos)
///
/// # Parameters
/// - `d`: chrono Duration
///
/// # Returns
/// (seconds, nanos) tuple
pub fn duration_to_parts(d: &Duration) -> (i64, i32) {
    let seconds = d.num_seconds();
    let nanos = (*d - Duration::seconds(seconds))
        .num_nanoseconds()
        .unwrap_or(0) as i32;
    (seconds, nanos)
}

/// Parse RFC3339 timestamp string
///
/// # Parameters
/// - `s`: RFC3339 string (e.g., "2023-05-28T00:00:00Z")
///
/// # Returns
/// - `Ok(DateTime)` if parse succeeds
/// - `Err(String)` with error message
pub fn parse_rfc3339(s: &str) -> Result<DateTime<FixedOffset>, String> {
    let dt = DateTime::parse_from_rfc3339(s).map_err(|e| e.to_string())?;

    // Validate year range per CEL spec: 0001-01-01 to 9999-12-31
    let year = dt.year();
    if !(1..=9999).contains(&year) {
        return Err(format!(
            "timestamp year {} is out of valid CEL range (0001-9999)",
            year
        ));
    }

    Ok(dt)
}

/// Parse duration string using CEL format
///
/// Supports CEL duration format:
/// - "1h30m" = 1 hour 30 minutes
/// - "1.5s" = 1.5 seconds  
/// - "-10h30m45.123s" = negative duration
///
/// # Implementation Note
/// This is vendored from cel-0.12's duration module since it's not publicly exported.
/// Uses nom parser from cel's transitive dependency (no additional deps needed).
/// If cel crate makes its duration module public in future, we can remove this vendored code.
///
/// # Parameters
/// - `s`: Duration string
///
/// # Returns
/// - `Ok(Duration)` if parse succeeds
/// - `Err(String)` with error message
pub fn parse_duration(s: &str) -> Result<Duration, String> {
    use nom::IResult;
    use nom::Parser;
    use nom::branch::alt;
    use nom::bytes::complete::tag;
    use nom::character::complete::char;
    use nom::combinator::{map, opt};
    use nom::multi::many1;
    use nom::number::complete::double;

    enum Unit {
        Nanosecond,
        Microsecond,
        Millisecond,
        Second,
        Minute,
        Hour,
    }

    fn parse_unit(i: &str) -> IResult<&str, Unit> {
        alt((
            map(tag("ms"), |_| Unit::Millisecond),
            map(tag("us"), |_| Unit::Microsecond),
            map(tag("ns"), |_| Unit::Nanosecond),
            map(char('h'), |_| Unit::Hour),
            map(char('m'), |_| Unit::Minute),
            map(char('s'), |_| Unit::Second),
        ))
        .parse(i)
    }

    fn to_duration(num: f64, unit: Unit) -> Result<Duration, String> {
        // Use different Duration constructors to avoid overflow when converting to nanoseconds
        let result = match unit {
            Unit::Nanosecond => Duration::nanoseconds(num.trunc() as i64),
            Unit::Microsecond => Duration::microseconds(num.trunc() as i64),
            Unit::Millisecond => Duration::milliseconds(num.trunc() as i64),
            Unit::Second => {
                // For seconds, handle fractional part separately to avoid overflow
                let secs = num.trunc() as i64;
                let nanos = ((num.fract() * 1_000_000_000.0).trunc() as i32).abs();
                match Duration::try_seconds(secs) {
                    Some(d) => {
                        if nanos == 0 {
                            d
                        } else {
                            d + Duration::nanoseconds(nanos as i64 * secs.signum())
                        }
                    }
                    None => return Err(format!("duration seconds overflow: {}", secs)),
                }
            }
            Unit::Minute => {
                let mins = num.trunc() as i64;
                match Duration::try_minutes(mins) {
                    Some(d) => d,
                    None => return Err(format!("duration minutes overflow: {}", mins)),
                }
            }
            Unit::Hour => {
                let hours = num.trunc() as i64;
                match Duration::try_hours(hours) {
                    Some(d) => d,
                    None => return Err(format!("duration hours overflow: {}", hours)),
                }
            }
        };
        Ok(result)
    }

    fn parse_number_unit(i: &str) -> IResult<&str, Duration> {
        let (i, num) = double(i)?;
        let (i, unit) = parse_unit(i)?;
        let duration = to_duration(num, unit).map_err(|_e| {
            nom::Err::Failure(nom::error::Error::new(i, nom::error::ErrorKind::Fail))
        })?;
        Ok((i, duration))
    }

    fn parse_negative(i: &str) -> IResult<&str, ()> {
        let (i, _): (&str, char) = char('-')(i)?;
        Ok((i, ()))
    }

    fn parse_duration_impl(i: &str) -> IResult<&str, Duration> {
        let (i, neg) = opt(parse_negative).parse(i)?;
        if i == "0" {
            return Ok((i, Duration::zero()));
        }
        let (i, duration) =
            many1(parse_number_unit)
                .parse(i)
                .map(|(i, d): (&str, Vec<Duration>)| {
                    (i, d.iter().fold(Duration::zero(), |acc, next| acc + *next))
                })?;
        Ok((i, if neg.is_some() { -duration } else { duration }))
    }

    match parse_duration_impl(s) {
        Ok((remaining, duration)) => {
            if !remaining.is_empty() {
                Err(format!(
                    "Unexpected characters after duration: {}",
                    remaining
                ))
            } else {
                // Validate duration range (CEL spec)
                // CEL uses a conservative limit slightly less than the max timestamp span
                const MIN_DURATION_SECONDS: i64 = -315_537_897_598;
                const MAX_DURATION_SECONDS: i64 = 315_537_897_598;

                let seconds = duration.num_seconds();
                if !(MIN_DURATION_SECONDS..=MAX_DURATION_SECONDS).contains(&seconds) {
                    return Err(format!(
                        "duration out of valid range (±{} seconds): {} seconds",
                        MAX_DURATION_SECONDS, seconds
                    ));
                }

                Ok(duration)
            }
        }
        Err(e) => Err(format!("Failed to parse duration: {}", e)),
    }
}

/// Format duration to CEL string format
///
/// Formats as "72h3m0.5s" with leading zero units omitted.
/// Durations less than 1s use smaller units (ms, us, ns).
/// Zero duration formats as "0s".
///
/// # Implementation Note
/// This is vendored from cel-0.12's duration module since it's not publicly exported.
/// If cel crate makes its duration module public in future, we can remove this vendored code.
///
/// # Parameters
/// - `d`: chrono Duration to format
///
/// # Returns
/// Formatted duration string
pub fn format_duration(d: &Duration) -> String {
    // Format duration according to CEL spec: seconds with optional fractional part
    // Examples: "0s", "1s", "1.5s", "0.000000001s", "-1.5s"

    // Get total nanoseconds, handling overflow for very large durations
    let (seconds, nanos) = if let Some(total_nanos) = d.num_nanoseconds() {
        let is_negative = total_nanos < 0;
        let abs_nanos = total_nanos.unsigned_abs();
        let secs = (abs_nanos / 1_000_000_000) as i64;
        let nanos = (abs_nanos % 1_000_000_000) as i64;
        if is_negative {
            (-secs, -nanos)
        } else {
            (secs, nanos)
        }
    } else {
        // Duration too large for nanoseconds, use seconds
        let secs = d.num_seconds();
        (secs, 0)
    };

    // Handle zero duration
    if seconds == 0 && nanos == 0 {
        return "0s".to_string();
    }

    // Format with sign
    let is_negative = seconds < 0 || nanos < 0;
    let abs_seconds = seconds.unsigned_abs();
    let abs_nanos = nanos.unsigned_abs();

    // Format fractional part (remove trailing zeros)
    let fractional = if abs_nanos > 0 {
        let mut frac_str = format!("{:09}", abs_nanos);
        // Trim trailing zeros
        while frac_str.ends_with('0') {
            frac_str.pop();
        }
        format!(".{}", frac_str)
    } else {
        String::new()
    };

    if is_negative {
        format!("-{}{}s", abs_seconds, fractional)
    } else {
        format!("{}{}s", abs_seconds, fractional)
    }
}

/// Timezone representation for CEL spec compliance
/// Supports "UTC", IANA timezone names, and fixed offsets
#[derive(Debug, Clone, Copy)]
pub enum Timezone {
    /// IANA timezone database name (e.g., "America/Los_Angeles")
    Iana(Tz),
    /// Fixed offset from UTC (e.g., "+05:30", "-08:00")
    Fixed(FixedOffset),
}

/// Parse timezone string according to CEL spec grammar:
/// TimeZone = "UTC" | LongTZ | FixedTZ
/// LongTZ = IANA timezone database name
/// FixedTZ = ( "+" | "-" )? Digit Digit ":" Digit Digit
///
/// Note: Fixed offset sign is optional; unsigned offsets are treated as positive.
///
/// # Parameters
/// - `tz_str`: Timezone string to parse
///
/// # Returns
/// - `Ok(Timezone)` if parsing succeeds
/// - `Err(String)` with error message if parsing fails
///
/// # Examples
/// ```
/// parse_timezone("UTC") // Ok(Timezone::Iana(Tz::UTC))
/// parse_timezone("America/Los_Angeles") // Ok(Timezone::Iana(...))
/// parse_timezone("+05:30") // Ok(Timezone::Fixed(...))
/// parse_timezone("-08:00") // Ok(Timezone::Fixed(...))
/// parse_timezone("02:00") // Ok(Timezone::Fixed(...)) - unsigned, treated as +02:00
/// ```
pub fn parse_timezone(tz_str: &str) -> Result<Timezone, String> {
    // Try parsing as IANA timezone name (includes "UTC")
    if let Ok(tz) = tz_str.parse::<Tz>() {
        return Ok(Timezone::Iana(tz));
    }

    // Try parsing as fixed offset: (+|-)HH:MM
    if let Some(offset) = parse_fixed_offset(tz_str) {
        return Ok(Timezone::Fixed(offset));
    }

    Err(format!(
        "Invalid timezone '{}': must be 'UTC', IANA name, or fixed offset (+/-HH:MM)",
        tz_str
    ))
}

/// Parse fixed offset timezone string in format (+|-)HH:MM or HH:MM
/// Returns Some(FixedOffset) if successful, None otherwise
/// If no sign is present, assumes positive offset
fn parse_fixed_offset(s: &str) -> Option<FixedOffset> {
    let bytes = s.as_bytes();

    // Check if string starts with sign or digit
    let (is_negative, offset) = match bytes.first()? {
        b'+' => {
            // Signed format: +HH:MM (6 characters)
            if s.len() != 6 {
                return None;
            }
            (false, 1)
        }
        b'-' => {
            // Signed format: -HH:MM (6 characters)
            if s.len() != 6 {
                return None;
            }
            (true, 1)
        }
        b'0'..=b'9' => {
            // Unsigned format: HH:MM (5 characters, assume positive)
            if s.len() != 5 {
                return None;
            }
            (false, 0)
        }
        _ => return None,
    };

    // Parse hours
    let hours = parse_two_digits(&bytes[offset..offset + 2])?;

    // Check colon separator
    if bytes[offset + 2] != b':' {
        return None;
    }

    // Parse minutes
    let minutes = parse_two_digits(&bytes[offset + 3..offset + 5])?;

    // Validate ranges
    if hours > 23 || minutes > 59 {
        return None;
    }

    // Calculate total offset in seconds
    let offset_seconds = (hours as i32 * 3600) + (minutes as i32 * 60);

    // Create FixedOffset
    if is_negative {
        FixedOffset::west_opt(offset_seconds)
    } else {
        FixedOffset::east_opt(offset_seconds)
    }
}

/// Parse exactly two decimal digits from byte slice
/// Returns Some(value) if successful, None otherwise
fn parse_two_digits(bytes: &[u8]) -> Option<u8> {
    if bytes.len() != 2 {
        return None;
    }

    let d1 = match bytes[0] {
        b'0'..=b'9' => bytes[0] - b'0',
        _ => return None,
    };

    let d2 = match bytes[1] {
        b'0'..=b'9' => bytes[1] - b'0',
        _ => return None,
    };

    Some(d1 * 10 + d2)
}
