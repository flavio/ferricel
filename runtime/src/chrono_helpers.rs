//! Chrono helper functions for duration and timestamp conversions.
//! Provides wrappers around chrono types for FFI boundary and parsing/formatting.

use chrono::{DateTime, Duration, FixedOffset, TimeZone, Utc};
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
    DateTime::parse_from_rfc3339(s).map_err(|e| e.to_string())
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
    use nom::branch::alt;
    use nom::bytes::complete::tag;
    use nom::character::complete::char;
    use nom::combinator::{map, opt};
    use nom::multi::many1;
    use nom::number::complete::double;
    use nom::IResult;

    enum Unit {
        Nanosecond,
        Microsecond,
        Millisecond,
        Second,
        Minute,
        Hour,
    }

    impl Unit {
        fn nanos(&self) -> i64 {
            match self {
                Unit::Nanosecond => 1,
                Unit::Microsecond => 1_000,
                Unit::Millisecond => 1_000_000,
                Unit::Second => 1_000_000_000,
                Unit::Minute => 60 * 1_000_000_000,
                Unit::Hour => 60 * 60 * 1_000_000_000,
            }
        }
    }

    fn parse_unit(i: &str) -> IResult<&str, Unit> {
        alt((
            map(tag("ms"), |_| Unit::Millisecond),
            map(tag("us"), |_| Unit::Microsecond),
            map(tag("ns"), |_| Unit::Nanosecond),
            map(char('h'), |_| Unit::Hour),
            map(char('m'), |_| Unit::Minute),
            map(char('s'), |_| Unit::Second),
        ))(i)
    }

    fn to_duration(num: f64, unit: Unit) -> Duration {
        Duration::nanoseconds((num * unit.nanos() as f64).trunc() as i64)
    }

    fn parse_number_unit(i: &str) -> IResult<&str, Duration> {
        let (i, num) = double(i)?;
        let (i, unit) = parse_unit(i)?;
        let duration = to_duration(num, unit);
        Ok((i, duration))
    }

    fn parse_negative(i: &str) -> IResult<&str, ()> {
        let (i, _): (&str, char) = char('-')(i)?;
        Ok((i, ()))
    }

    fn parse_duration_impl(i: &str) -> IResult<&str, Duration> {
        let (i, neg) = opt(parse_negative)(i)?;
        if i == "0" {
            return Ok((i, Duration::zero()));
        }
        let (i, duration) = many1(parse_number_unit)(i)
            .map(|(i, d)| (i, d.iter().fold(Duration::zero(), |acc, next| acc + *next)))?;
        Ok((i, duration * if neg.is_some() { -1 } else { 1 }))
    }

    match parse_duration_impl(s) {
        Ok((remaining, duration)) => {
            if !remaining.is_empty() {
                Err(format!(
                    "Unexpected characters after duration: {}",
                    remaining
                ))
            } else {
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
    const SECOND: u64 = 1_000_000_000;
    const MILLISECOND: u64 = 1_000_000;
    const MICROSECOND: u64 = 1_000;

    fn format_float(buf: &mut [u8], mut v: u64, prec: usize) -> (usize, u64) {
        let mut w = buf.len();
        let mut print = false;
        for _ in 0..prec {
            let digit = v % 10;
            print = print || digit != 0;
            if print {
                w -= 1;
                buf[w] = digit as u8 + b'0';
            }
            v /= 10;
        }
        if print {
            w -= 1;
            buf[w] = b'.';
        }
        (w, v)
    }

    fn format_int(buf: &mut [u8], mut v: u64) -> usize {
        let mut w = buf.len();
        if v == 0 {
            w -= 1;
            buf[w] = b'0';
        } else {
            while v > 0 {
                w -= 1;
                buf[w] = (v % 10) as u8 + b'0';
                v /= 10;
            }
        }
        w
    }

    let buf = &mut [0u8; 32];
    let mut w = buf.len();

    let mut neg = false;
    let mut u = d
        .num_nanoseconds()
        .map(|n| {
            if n < 0 {
                neg = true;
            }
            n.unsigned_abs()
        })
        .unwrap_or_else(|| {
            let s = d.num_seconds();
            if s < 0 {
                neg = true;
            }
            s.unsigned_abs() * SECOND
        });

    if u < SECOND {
        // Special case: if duration is smaller than a second,
        // use smaller units, like 1.2ms
        let mut _prec = 0;
        w -= 1;
        buf[w] = b's';
        w -= 1;

        if u == 0 {
            return "0s".to_string();
        } else if u < MICROSECOND {
            _prec = 0;
            buf[w] = b'n';
        } else if u < MILLISECOND {
            _prec = 3;
            // U+00B5 'µ' micro sign == 0xC2 0xB5
            buf[w] = 0xB5;
            w -= 1;
            buf[w] = 0xC2;
        } else {
            _prec = 6;
            buf[w] = b'm';
        }
        (w, u) = format_float(&mut buf[..w], u, _prec);
        w = format_int(&mut buf[..w], u);
    } else {
        w -= 1;
        buf[w] = b's';
        (w, u) = format_float(&mut buf[..w], u, 9);

        // u is now integer number of seconds
        w = format_int(&mut buf[..w], u % 60);
        u /= 60;

        // u is now integer number of minutes
        if u > 0 {
            w -= 1;
            buf[w] = b'm';
            w = format_int(&mut buf[..w], u % 60);
            u /= 60;

            // u is now integer number of hours
            if u > 0 {
                w -= 1;
                buf[w] = b'h';
                w = format_int(&mut buf[..w], u);
            }
        }
    }

    if neg {
        w -= 1;
        buf[w] = b'-';
    }
    String::from_utf8_lossy(&buf[w..]).into_owned()
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
