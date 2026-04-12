//! CEL extended string library functions.
//!
//! Implements the functions defined in the CEL `strings` extension library:
//! `charAt`, `indexOf`, `lastIndexOf`, `lowerAscii`, `upperAscii`, `replace`,
//! `split`, `substring`, `trim`, `reverse`, `format`, and `strings.quote`.

use crate::types::{CelMapKey, CelValue};

// ─── helpers ──────────────────────────────────────────────────────────────────

/// Convert a codepoint offset into a byte offset.  Returns `None` if `cp_offset`
/// exceeds the number of codepoints in the string.
pub(crate) fn cp_to_byte_offset(s: &str, cp_offset: usize) -> Option<usize> {
    if cp_offset == 0 {
        return Some(0);
    }
    s.char_indices()
        .nth(cp_offset)
        .map(|(byte_idx, _)| byte_idx)
        // If the offset equals the codepoint count, return the byte length
        .or_else(|| {
            if cp_offset == s.chars().count() {
                Some(s.len())
            } else {
                None
            }
        })
}

/// Returns a static type name string for a CelValue (used in error messages).
fn cel_type_name(val: &CelValue) -> &'static str {
    match val {
        CelValue::Null => "null",
        CelValue::Bool(_) => "bool",
        CelValue::Int(_) => "int",
        CelValue::UInt(_) => "uint",
        CelValue::Double(_) => "double",
        CelValue::String(_) => "string",
        CelValue::Bytes(_) => "bytes",
        CelValue::Array(_) => "list",
        CelValue::Object(_) => "map",
        CelValue::Timestamp(_) => "timestamp",
        CelValue::Duration(_) => "duration",
        CelValue::Type(_) => "type",
        CelValue::Error(_) => "error",
        CelValue::Url(_, _) => "url",
        CelValue::IpAddr(_) => "net.IP",
        CelValue::Cidr(_, _) => "net.CIDR",
        CelValue::Semver(_) => "semver",
        CelValue::Quantity(_) => "quantity",
        CelValue::Optional(_) => "optional_type",
    }
}

/// Find the first codepoint index of `sub` in `s` starting from codepoint offset `start`.
/// Returns -1 if not found.
pub(crate) fn find_index_of(s: &str, sub: &str, start: usize) -> i64 {
    // Convert start codepoint offset to byte offset
    let byte_start = match cp_to_byte_offset(s, start) {
        Some(b) => b,
        None => return -1,
    };
    let search_slice = &s[byte_start..];

    if sub.is_empty() {
        // Empty needle: return start (codepoint offset)
        return start as i64;
    }

    // Find byte position within the search slice
    match search_slice.find(sub) {
        Some(byte_offset) => {
            // Convert byte offset (relative to byte_start) back to codepoint offset
            let abs_byte = byte_start + byte_offset;
            s[..abs_byte].chars().count() as i64
        }
        None => -1,
    }
}

/// Find the last codepoint index of `sub` in `s` where the match starts at or before
/// codepoint `end_offset`. Returns -1 if not found.
pub(crate) fn find_last_index_of(s: &str, sub: &str, end_offset: i64) -> i64 {
    let cp_len = s.chars().count() as i64;

    if sub.is_empty() {
        // Empty needle: clamp to string length
        let clamped = end_offset.min(cp_len);
        return clamped;
    }

    // Clamp the end offset to the string length
    let end_cp = end_offset.min(cp_len) as usize;

    // A match starting at codepoint `end_cp` can extend up to
    // `end_cp + sub_cp_len` codepoints into the string. We compute the byte
    // offset of that position so that rfind finds all matches that start at or
    // before `end_cp`.
    let sub_cp_len = sub.chars().count();
    let search_end_cp = (end_cp + sub_cp_len).min(cp_len as usize);
    let search_end_byte = cp_to_byte_offset(s, search_end_cp).unwrap_or(s.len());
    let search_slice = &s[..search_end_byte];

    match search_slice.rfind(sub) {
        Some(byte_offset) => s[..byte_offset].chars().count() as i64,
        None => -1,
    }
}

/// Returns true if a `CelValue::Object` represents a proto message (has a `__type__` key).
fn is_proto_message(entries: &std::collections::HashMap<CelMapKey, CelValue>) -> bool {
    entries.contains_key(&CelMapKey::String("__type__".into()))
}

/// Format a CEL value as a string for use in `%s`.
///
/// Returns `Err` if the value is (or contains) a proto message object, because
/// the CEL spec forbids formatting proto messages with the `%s` verb.
fn format_value_as_string(val: &CelValue) -> Result<String, String> {
    match val {
        CelValue::String(s) => Ok(s.clone()),
        CelValue::Int(i) => Ok(i.to_string()),
        CelValue::UInt(u) => Ok(u.to_string()),
        CelValue::Double(d) => Ok(format_double_s(*d)),
        CelValue::Bool(b) => Ok(b.to_string()),
        CelValue::Null => Ok("null".to_string()),
        CelValue::Bytes(b) => {
            // bytes format as UTF-8 string if valid, otherwise hex
            Ok(String::from_utf8_lossy(b).into_owned())
        }
        CelValue::Array(items) => {
            let mut parts = Vec::with_capacity(items.len());
            for item in items.iter() {
                parts.push(format_value_as_string(item)?);
            }
            Ok(format!("[{}]", parts.join(", ")))
        }
        CelValue::Object(entries) => {
            // Proto messages (objects with __type__) are not allowed in %s
            if is_proto_message(entries) {
                return Err("format: object is not allowed in string clause".to_string());
            }
            // Plain CEL maps: sort by key for deterministic output
            let mut pairs = Vec::with_capacity(entries.len());
            for (k, v) in entries.iter() {
                pairs.push(format!(
                    "{}: {}",
                    k.to_string_key(),
                    format_value_as_string(v)?
                ));
            }
            pairs.sort();
            Ok(format!("{{{}}}", pairs.join(", ")))
        }
        CelValue::Timestamp(ts) => {
            // Format as RFC3339 with Z for UTC
            if ts.offset().local_minus_utc() == 0 {
                Ok(ts.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            } else {
                Ok(ts.to_rfc3339())
            }
        }
        CelValue::Duration(d) => {
            // Format as seconds with 's' suffix (matching CEL spec)
            Ok(format!("{}s", d.num_seconds()))
        }
        CelValue::Type(t) => Ok(t.clone()),
        _ => Ok(String::new()),
    }
}

/// Format a double for `%s`: omit trailing zeros but keep at least one decimal.
fn format_double_s(d: f64) -> String {
    if d.is_nan() {
        return "NaN".to_string();
    }
    if d.is_infinite() {
        return if d > 0.0 {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        };
    }
    // Use Rust's default f64 Display which gives the shortest round-trip representation
    format!("{d}")
}

/// Core format string processor.
fn format_string(fmt: &str, args: &[CelValue]) -> Result<String, String> {
    let mut out = String::with_capacity(fmt.len());
    let mut chars = fmt.chars().peekable();
    let mut arg_idx = 0usize;

    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // We have a '%' — peek at next char
        match chars.next() {
            None => return Err("format: trailing '%'".to_string()),
            Some('%') => {
                out.push('%');
            }
            Some(next) => {
                // Check for optional precision: %.Nverb
                let (precision, verb) = if next == '.' {
                    // Read digits for precision
                    let mut prec_str = String::new();
                    loop {
                        match chars.peek() {
                            Some(&d) if d.is_ascii_digit() => {
                                prec_str.push(d);
                                chars.next();
                            }
                            _ => break,
                        }
                    }
                    let prec: usize = prec_str.parse().unwrap_or(0);
                    let v = chars.next().ok_or("format: truncated format string")?;
                    (Some(prec), v)
                } else {
                    (None, next)
                };

                if arg_idx >= args.len() {
                    return Err(format!("format: not enough arguments (need arg {arg_idx})"));
                }
                let arg = &args[arg_idx];
                arg_idx += 1;

                match verb {
                    's' => out.push_str(&format_value_as_string(arg)?),
                    'd' => out.push_str(&format_arg_decimal(arg)?),
                    'f' => out.push_str(&format_arg_fixed(arg, precision.unwrap_or(6))?),
                    'e' => out.push_str(&format_arg_sci(arg, precision.unwrap_or(6))?),
                    'b' => out.push_str(&format_arg_binary(arg)?),
                    'o' => out.push_str(&format_arg_octal(arg)?),
                    'x' => out.push_str(&format_arg_hex(arg, false)?),
                    'X' => out.push_str(&format_arg_hex(arg, true)?),
                    other => {
                        return Err(format!(
                            "could not parse formatting clause: unrecognized formatting clause \"{other}\""
                        ));
                    }
                }
            }
        }
    }
    Ok(out)
}

fn format_arg_decimal(val: &CelValue) -> Result<String, String> {
    match val {
        CelValue::Int(i) => Ok(i.to_string()),
        CelValue::UInt(u) => Ok(u.to_string()),
        CelValue::Double(d) => {
            if d.is_nan() {
                return Ok("NaN".to_string());
            }
            if d.is_infinite() {
                return Ok(if *d > 0.0 {
                    "Infinity".to_string()
                } else {
                    "-Infinity".to_string()
                });
            }
            Ok((*d as i64).to_string())
        }
        _ => Err(format!(
            "format: %%d expects int/uint/double, got {}",
            cel_type_name(val)
        )),
    }
}

fn format_arg_fixed(val: &CelValue, prec: usize) -> Result<String, String> {
    let d = to_float(val)?;
    if d.is_nan() {
        return Ok("NaN".to_string());
    }
    if d.is_infinite() {
        return Ok(if d > 0.0 {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        });
    }
    Ok(format!("{d:.prec$}"))
}

fn format_arg_sci(val: &CelValue, prec: usize) -> Result<String, String> {
    let d = to_float(val)?;
    if d.is_nan() {
        return Ok("NaN".to_string());
    }
    if d.is_infinite() {
        return Ok(if d > 0.0 {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        });
    }
    // Rust's {:e} uses lowercase 'e', and produces e.g. "1.052033e3" (no leading zero in exponent)
    // CEL spec expects "1.052033e+03" — two-digit exponent with sign
    let raw = format!("{d:.prec$e}");
    // Normalise exponent: e3 → e+03, e-3 → e-03
    Ok(normalise_sci_exponent(&raw))
}

fn format_arg_binary(val: &CelValue) -> Result<String, String> {
    match val {
        CelValue::Int(i) => Ok(format!("{i:b}")),
        CelValue::UInt(u) => Ok(format!("{u:b}")),
        CelValue::Bool(b) => Ok(if *b { "1".to_string() } else { "0".to_string() }),
        CelValue::Bytes(b) => {
            let s: String = b.iter().map(|byte| format!("{byte:08b}")).collect();
            Ok(s)
        }
        _ => Err(format!(
            "format: %%b expects int/uint/bool/bytes, got {}",
            cel_type_name(val)
        )),
    }
}

fn format_arg_octal(val: &CelValue) -> Result<String, String> {
    match val {
        CelValue::Int(i) => Ok(format!("{i:o}")),
        CelValue::UInt(u) => Ok(format!("{u:o}")),
        _ => Err(format!(
            "format: %%o expects int/uint, got {}",
            cel_type_name(val)
        )),
    }
}

fn format_arg_hex(val: &CelValue, upper: bool) -> Result<String, String> {
    match val {
        CelValue::Int(i) => {
            if upper {
                Ok(format!("{i:X}"))
            } else {
                Ok(format!("{i:x}"))
            }
        }
        CelValue::UInt(u) => {
            if upper {
                Ok(format!("{u:X}"))
            } else {
                Ok(format!("{u:x}"))
            }
        }
        CelValue::String(s) => {
            if upper {
                Ok(s.bytes().map(|b| format!("{b:02X}")).collect())
            } else {
                Ok(s.bytes().map(|b| format!("{b:02x}")).collect())
            }
        }
        CelValue::Bytes(b) => {
            if upper {
                Ok(b.iter().map(|byte| format!("{byte:02X}")).collect())
            } else {
                Ok(b.iter().map(|byte| format!("{byte:02x}")).collect())
            }
        }
        _ => Err(format!(
            "format: %%x/X expects int/uint/string/bytes, got {}",
            cel_type_name(val)
        )),
    }
}

fn to_float(val: &CelValue) -> Result<f64, String> {
    match val {
        CelValue::Double(d) => Ok(*d),
        CelValue::Int(i) => Ok(*i as f64),
        CelValue::UInt(u) => Ok(*u as f64),
        _ => Err(format!(
            "format: %%f/e expects numeric, got {}",
            cel_type_name(val)
        )),
    }
}

/// Normalise a Rust scientific notation exponent (e.g. `1e3` → `1e+03`, `1e-3` → `1e-03`).
fn normalise_sci_exponent(s: &str) -> String {
    // Find the 'e' or 'E' separator
    let e_pos = s.find(['e', 'E']);
    let e_pos = match e_pos {
        Some(p) => p,
        None => return s.to_string(),
    };
    let (mantissa, exp_part) = s.split_at(e_pos);
    let e_char = &exp_part[..1]; // 'e' or 'E'
    let exp_digits = &exp_part[1..];
    // exp_digits is something like "3", "-3", "+3"
    let (sign, digits) = if let Some(stripped) = exp_digits.strip_prefix('+') {
        ("+", stripped)
    } else if let Some(stripped) = exp_digits.strip_prefix('-') {
        ("-", stripped)
    } else {
        ("+", exp_digits)
    };
    // Zero-pad to at least 2 digits
    format!("{mantissa}{e_char}{sign}{digits:0>2}")
}

// ─── Public extern "C" functions ──────────────────────────────────────────────

/// Returns the character at codepoint index `i` as a string.
/// Returns an empty string when `i` equals the string length (end sentinel).
/// Returns an error CelValue when `i` is out of range.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_char_at(
    string_ptr: *const CelValue,
    index_ptr: *const CelValue,
) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "charAt: receiver is not a string".to_string(),
            )));
        }
    };
    let idx = match unsafe { &*index_ptr } {
        CelValue::Int(i) => *i,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "charAt: index is not an int".to_string(),
            )));
        }
    };
    if idx < 0 {
        return Box::into_raw(Box::new(CelValue::Error(
            "charAt: index out of range".to_string(),
        )));
    }
    let idx = idx as usize;
    let len = s.chars().count();
    if idx == len {
        return Box::into_raw(Box::new(CelValue::String(String::new())));
    }
    match s.chars().nth(idx) {
        Some(c) => Box::into_raw(Box::new(CelValue::String(c.to_string()))),
        None => Box::into_raw(Box::new(CelValue::Error(
            "charAt: index out of range".to_string(),
        ))),
    }
}

/// Returns the first codepoint index of `sub` in `s`, or -1 if not found.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_index_of(
    string_ptr: *const CelValue,
    sub_ptr: *const CelValue,
) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "indexOf: receiver is not a string".to_string(),
            )));
        }
    };
    let sub = match unsafe { &*sub_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "indexOf: argument is not a string".to_string(),
            )));
        }
    };
    let result = find_index_of(&s, &sub, 0);
    Box::into_raw(Box::new(CelValue::Int(result)))
}

/// Returns the first codepoint index of `sub` in `s` starting from codepoint offset, or -1.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_index_of_offset(
    string_ptr: *const CelValue,
    sub_ptr: *const CelValue,
    offset_ptr: *const CelValue,
) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "indexOf: receiver is not a string".to_string(),
            )));
        }
    };
    let sub = match unsafe { &*sub_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "indexOf: argument is not a string".to_string(),
            )));
        }
    };
    let offset = match unsafe { &*offset_ptr } {
        CelValue::Int(i) => *i,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "indexOf: offset is not an int".to_string(),
            )));
        }
    };
    if offset < 0 {
        return Box::into_raw(Box::new(CelValue::Error(
            "indexOf: offset out of range".to_string(),
        )));
    }
    let cp_len = s.chars().count() as i64;
    if offset > cp_len {
        return Box::into_raw(Box::new(CelValue::Error(format!(
            "index out of range: {}",
            offset
        ))));
    }
    let result = find_index_of(&s, &sub, offset as usize);
    Box::into_raw(Box::new(CelValue::Int(result)))
}

/// Returns the last codepoint index of `sub` in `s`, or -1 if not found.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_last_index_of(
    string_ptr: *const CelValue,
    sub_ptr: *const CelValue,
) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "lastIndexOf: receiver is not a string".to_string(),
            )));
        }
    };
    let sub = match unsafe { &*sub_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "lastIndexOf: argument is not a string".to_string(),
            )));
        }
    };
    let cp_len = s.chars().count() as i64;
    let result = find_last_index_of(&s, &sub, cp_len);
    Box::into_raw(Box::new(CelValue::Int(result)))
}

/// Returns the last codepoint index of `sub` in `s` ending at or before `offset`, or -1.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_last_index_of_offset(
    string_ptr: *const CelValue,
    sub_ptr: *const CelValue,
    offset_ptr: *const CelValue,
) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "lastIndexOf: receiver is not a string".to_string(),
            )));
        }
    };
    let sub = match unsafe { &*sub_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "lastIndexOf: argument is not a string".to_string(),
            )));
        }
    };
    let offset = match unsafe { &*offset_ptr } {
        CelValue::Int(i) => *i,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "lastIndexOf: offset is not an int".to_string(),
            )));
        }
    };
    if offset < 0 {
        return Box::into_raw(Box::new(CelValue::Error(
            "lastIndexOf: offset out of range".to_string(),
        )));
    }
    let cp_len = s.chars().count() as i64;
    if offset > cp_len {
        return Box::into_raw(Box::new(CelValue::Error(format!(
            "index out of range: {}",
            offset
        ))));
    }
    let result = find_last_index_of(&s, &sub, offset);
    Box::into_raw(Box::new(CelValue::Int(result)))
}

/// Lowercases only ASCII characters, leaving non-ASCII codepoints unchanged.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_lower_ascii(string_ptr: *const CelValue) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "lowerAscii: receiver is not a string".to_string(),
            )));
        }
    };
    let result: String = s.chars().map(|c| c.to_ascii_lowercase()).collect();
    Box::into_raw(Box::new(CelValue::String(result)))
}

/// Uppercases only ASCII characters, leaving non-ASCII codepoints unchanged.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_upper_ascii(string_ptr: *const CelValue) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "upperAscii: receiver is not a string".to_string(),
            )));
        }
    };
    let result: String = s.chars().map(|c| c.to_ascii_uppercase()).collect();
    Box::into_raw(Box::new(CelValue::String(result)))
}

/// Replaces all occurrences of `old` with `new` in `s`.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_replace(
    string_ptr: *const CelValue,
    old_ptr: *const CelValue,
    new_ptr: *const CelValue,
) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "replace: receiver is not a string".to_string(),
            )));
        }
    };
    let old = match unsafe { &*old_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "replace: 'old' is not a string".to_string(),
            )));
        }
    };
    let new = match unsafe { &*new_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "replace: 'new' is not a string".to_string(),
            )));
        }
    };
    Box::into_raw(Box::new(CelValue::String(
        s.replace(old.as_str(), new.as_str()),
    )))
}

/// Replaces up to `n` occurrences of `old` with `new` in `s`.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_replace_n(
    string_ptr: *const CelValue,
    old_ptr: *const CelValue,
    new_ptr: *const CelValue,
    n_ptr: *const CelValue,
) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "replace: receiver is not a string".to_string(),
            )));
        }
    };
    let old = match unsafe { &*old_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "replace: 'old' is not a string".to_string(),
            )));
        }
    };
    let new = match unsafe { &*new_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "replace: 'new' is not a string".to_string(),
            )));
        }
    };
    let n = match unsafe { &*n_ptr } {
        CelValue::Int(i) => *i,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "replace: count is not an int".to_string(),
            )));
        }
    };
    if n < 0 {
        return Box::into_raw(Box::new(CelValue::Error(
            "replace: count must be non-negative".to_string(),
        )));
    }
    Box::into_raw(Box::new(CelValue::String(s.replacen(
        old.as_str(),
        new.as_str(),
        n as usize,
    ))))
}

/// Splits `s` on `sep`, returning a list of strings.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_split(
    string_ptr: *const CelValue,
    sep_ptr: *const CelValue,
) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "split: receiver is not a string".to_string(),
            )));
        }
    };
    let sep = match unsafe { &*sep_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "split: separator is not a string".to_string(),
            )));
        }
    };
    let parts: Vec<CelValue> = s
        .split(sep.as_str())
        .map(|p| CelValue::String(p.to_string()))
        .collect();
    Box::into_raw(Box::new(CelValue::Array(parts)))
}

/// Splits `s` on `sep` with at most `n` parts.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_split_n(
    string_ptr: *const CelValue,
    sep_ptr: *const CelValue,
    n_ptr: *const CelValue,
) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "split: receiver is not a string".to_string(),
            )));
        }
    };
    let sep = match unsafe { &*sep_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "split: separator is not a string".to_string(),
            )));
        }
    };
    let n = match unsafe { &*n_ptr } {
        CelValue::Int(i) => *i,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "split: limit is not an int".to_string(),
            )));
        }
    };
    if n < -1 {
        return Box::into_raw(Box::new(CelValue::Error(
            "split: limit must be >= -1".to_string(),
        )));
    }
    // n == -1 means unlimited (split on all occurrences)
    let parts: Vec<CelValue> = if n == -1 {
        s.split(sep.as_str())
            .map(|p| CelValue::String(p.to_string()))
            .collect()
    } else {
        s.splitn(n as usize, sep.as_str())
            .map(|p| CelValue::String(p.to_string()))
            .collect()
    };
    Box::into_raw(Box::new(CelValue::Array(parts)))
}

/// Returns the substring of `s` from codepoint `start` to the end.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_substring(
    string_ptr: *const CelValue,
    start_ptr: *const CelValue,
) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "substring: receiver is not a string".to_string(),
            )));
        }
    };
    let start = match unsafe { &*start_ptr } {
        CelValue::Int(i) => *i,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "substring: start is not an int".to_string(),
            )));
        }
    };
    if start < 0 {
        return Box::into_raw(Box::new(CelValue::Error(
            "substring: index out of range".to_string(),
        )));
    }
    let start = start as usize;
    let cp_len = s.chars().count();
    if start > cp_len {
        return Box::into_raw(Box::new(CelValue::Error(
            "substring: index out of range".to_string(),
        )));
    }
    let result: String = s.chars().skip(start).collect();
    Box::into_raw(Box::new(CelValue::String(result)))
}

/// Returns the substring of `s` from codepoint `start` (inclusive) to `end` (exclusive).
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_substring_range(
    string_ptr: *const CelValue,
    start_ptr: *const CelValue,
    end_ptr: *const CelValue,
) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "substring: receiver is not a string".to_string(),
            )));
        }
    };
    let start = match unsafe { &*start_ptr } {
        CelValue::Int(i) => *i,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "substring: start is not an int".to_string(),
            )));
        }
    };
    let end = match unsafe { &*end_ptr } {
        CelValue::Int(i) => *i,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "substring: end is not an int".to_string(),
            )));
        }
    };
    if start < 0 || end < 0 {
        return Box::into_raw(Box::new(CelValue::Error(
            "substring: index out of range".to_string(),
        )));
    }
    let start = start as usize;
    let end = end as usize;
    let cp_len = s.chars().count();
    if start > cp_len || end > cp_len {
        return Box::into_raw(Box::new(CelValue::Error(
            "substring: index out of range".to_string(),
        )));
    }
    if end < start {
        return Box::into_raw(Box::new(CelValue::Error(
            "substring: end index must be >= start index".to_string(),
        )));
    }
    let result: String = s.chars().skip(start).take(end - start).collect();
    Box::into_raw(Box::new(CelValue::String(result)))
}

/// Trims leading and trailing Unicode whitespace.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_trim(string_ptr: *const CelValue) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "trim: receiver is not a string".to_string(),
            )));
        }
    };
    Box::into_raw(Box::new(CelValue::String(s.trim().to_string())))
}

/// Reverses the string by Unicode codepoints.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_reverse(string_ptr: *const CelValue) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "reverse: receiver is not a string".to_string(),
            )));
        }
    };
    let result: String = s.chars().rev().collect();
    Box::into_raw(Box::new(CelValue::String(result)))
}

/// `strings.quote(s)` — wraps the string in double quotes, escaping control
/// characters, backslashes, and double quotes.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_strings_quote(string_ptr: *const CelValue) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "strings.quote: argument is not a string".to_string(),
            )));
        }
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\x07' => out.push_str("\\a"),
            '\x08' => out.push_str("\\b"),
            '\x0C' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x0B' => out.push_str("\\v"),
            // All other characters (including printable unicode) pass through
            _ => out.push(c),
        }
    }
    out.push('"');
    Box::into_raw(Box::new(CelValue::String(out)))
}

/// `str.format(args)` — CEL string interpolation.
///
/// Supported verbs: `%s`, `%d`, `%f`, `%e`, `%b`, `%o`, `%x`, `%X`, `%%`.
/// Optional precision for float verbs: `%.Nf`, `%.Ne`.
///
/// # Safety
///
/// Caller must ensure all pointer arguments point to valid `CelValue` instances
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_string_format(
    string_ptr: *const CelValue,
    args_ptr: *const CelValue,
) -> *mut CelValue {
    let fmt = match unsafe { &*string_ptr } {
        CelValue::String(s) => s.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "format: receiver is not a string".to_string(),
            )));
        }
    };
    let args = match unsafe { &*args_ptr } {
        CelValue::Array(v) => v.clone(),
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "format: argument is not a list".to_string(),
            )));
        }
    };

    match format_string(&fmt, &args) {
        Ok(s) => Box::into_raw(Box::new(CelValue::String(s))),
        Err(e) => Box::into_raw(Box::new(CelValue::Error(e))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deserialization::cel_free_value;
    use rstest::rstest;

    // ── pure helpers ──────────────────────────────────────────────────────────

    #[rstest]
    #[case::zero_offset("hello", 0, Some(0))]
    #[case::ascii_mid("hello", 3, Some(3))]
    #[case::end_sentinel("hello", 5, Some(5))]
    #[case::out_of_range("hello", 6, None)]
    #[case::multibyte_mid("café", 2, Some(2))] // 'c','a' = 2 bytes, 'f' starts at byte 2
    #[case::multibyte_end("café", 4, Some(5))] // 4 codepoints = 5 bytes total ('é' is 2 bytes)
    #[case::empty_zero("", 0, Some(0))]
    #[case::empty_out_of_range("", 1, None)]
    fn test_cp_to_byte_offset(#[case] s: &str, #[case] cp: usize, #[case] expected: Option<usize>) {
        assert_eq!(cp_to_byte_offset(s, cp), expected);
    }

    #[rstest]
    #[case::basic("tacocat", "ac", 0, 1)]
    #[case::not_found("tacocat", "none", 0, -1)]
    #[case::empty_needle("tacocat", "", 0, 0)]
    #[case::with_offset("tacocat", "a", 3, 5)]
    #[case::unicode("ta©o©αT", "©", 0, 2)]
    #[case::offset_past_end("hello", "x", 10, -1)]
    fn test_find_index_of(
        #[case] s: &str,
        #[case] sub: &str,
        #[case] start: usize,
        #[case] expected: i64,
    ) {
        assert_eq!(find_index_of(s, sub, start), expected);
    }

    #[rstest]
    #[case::basic("tacocat", "at", 7, 5)]
    #[case::not_found("tacocat", "none", 7, -1)]
    #[case::empty_needle("tacocat", "", 7, 7)]
    #[case::with_offset("tacocat", "a", 3, 1)]
    #[case::empty_needle_clamped("tacocat", "", 100, 7)]
    fn test_find_last_index_of(
        #[case] s: &str,
        #[case] sub: &str,
        #[case] end_offset: i64,
        #[case] expected: i64,
    ) {
        assert_eq!(find_last_index_of(s, sub, end_offset), expected);
    }

    // ── charAt ────────────────────────────────────────────────────────────────

    #[rstest]
    #[case::first("hello", 0, "h")]
    #[case::mid("hello", 1, "e")]
    #[case::last("hello", 4, "o")]
    #[case::end_sentinel("hello", 5, "")]
    #[case::unicode("café", 3, "é")]
    fn test_char_at(#[case] s: &str, #[case] idx: i64, #[case] expected: &str) {
        let string_val = CelValue::String(s.to_string());
        let index_val = CelValue::Int(idx);
        unsafe {
            let result_ptr = cel_string_char_at(
                &string_val as *const CelValue,
                &index_val as *const CelValue,
            );
            let result = &*result_ptr;
            assert_eq!(result, &CelValue::String(expected.to_string()));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_char_at_out_of_range_returns_error() {
        let string_val = CelValue::String("hello".to_string());
        let index_val = CelValue::Int(99);
        unsafe {
            let result_ptr = cel_string_char_at(
                &string_val as *const CelValue,
                &index_val as *const CelValue,
            );
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_char_at_negative_index_returns_error() {
        let string_val = CelValue::String("hello".to_string());
        let index_val = CelValue::Int(-1);
        unsafe {
            let result_ptr = cel_string_char_at(
                &string_val as *const CelValue,
                &index_val as *const CelValue,
            );
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }

    // ── indexOf ───────────────────────────────────────────────────────────────

    #[rstest]
    #[case::basic("tacocat", "ac", 1)]
    #[case::not_found("tacocat", "none", -1)]
    #[case::empty_needle("tacocat", "", 0)]
    #[case::unicode("ta©o©αT", "©", 2)]
    fn test_index_of(#[case] s: &str, #[case] sub: &str, #[case] expected: i64) {
        let string_val = CelValue::String(s.to_string());
        let sub_val = CelValue::String(sub.to_string());
        unsafe {
            let result_ptr =
                cel_string_index_of(&string_val as *const CelValue, &sub_val as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::Int(expected));
            cel_free_value(result_ptr);
        }
    }

    #[rstest]
    #[case::with_offset("tacocat", "a", 3, 5)]
    #[case::offset_zero("tacocat", "a", 0, 1)]
    fn test_index_of_offset(
        #[case] s: &str,
        #[case] sub: &str,
        #[case] offset: i64,
        #[case] expected: i64,
    ) {
        let string_val = CelValue::String(s.to_string());
        let sub_val = CelValue::String(sub.to_string());
        let offset_val = CelValue::Int(offset);
        unsafe {
            let result_ptr = cel_string_index_of_offset(
                &string_val as *const CelValue,
                &sub_val as *const CelValue,
                &offset_val as *const CelValue,
            );
            assert_eq!(&*result_ptr, &CelValue::Int(expected));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_index_of_offset_negative_returns_error() {
        let string_val = CelValue::String("hello".to_string());
        let sub_val = CelValue::String("l".to_string());
        let offset_val = CelValue::Int(-1);
        unsafe {
            let result_ptr = cel_string_index_of_offset(
                &string_val as *const CelValue,
                &sub_val as *const CelValue,
                &offset_val as *const CelValue,
            );
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_index_of_offset_exceeds_length_returns_error() {
        let string_val = CelValue::String("hello".to_string());
        let sub_val = CelValue::String("l".to_string());
        let offset_val = CelValue::Int(99);
        unsafe {
            let result_ptr = cel_string_index_of_offset(
                &string_val as *const CelValue,
                &sub_val as *const CelValue,
                &offset_val as *const CelValue,
            );
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }

    // ── lastIndexOf ───────────────────────────────────────────────────────────

    #[rstest]
    #[case::basic("tacocat", "at", 5)]
    #[case::not_found("tacocat", "none", -1)]
    #[case::empty_needle("tacocat", "", 7)]
    fn test_last_index_of(#[case] s: &str, #[case] sub: &str, #[case] expected: i64) {
        let string_val = CelValue::String(s.to_string());
        let sub_val = CelValue::String(sub.to_string());
        unsafe {
            let result_ptr = cel_string_last_index_of(
                &string_val as *const CelValue,
                &sub_val as *const CelValue,
            );
            assert_eq!(&*result_ptr, &CelValue::Int(expected));
            cel_free_value(result_ptr);
        }
    }

    #[rstest]
    #[case::with_offset("tacocat", "a", 3, 1)]
    #[case::full_offset("tacocat", "a", 7, 5)]
    fn test_last_index_of_offset(
        #[case] s: &str,
        #[case] sub: &str,
        #[case] offset: i64,
        #[case] expected: i64,
    ) {
        let string_val = CelValue::String(s.to_string());
        let sub_val = CelValue::String(sub.to_string());
        let offset_val = CelValue::Int(offset);
        unsafe {
            let result_ptr = cel_string_last_index_of_offset(
                &string_val as *const CelValue,
                &sub_val as *const CelValue,
                &offset_val as *const CelValue,
            );
            assert_eq!(&*result_ptr, &CelValue::Int(expected));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_last_index_of_offset_negative_returns_error() {
        let string_val = CelValue::String("hello".to_string());
        let sub_val = CelValue::String("l".to_string());
        let offset_val = CelValue::Int(-1);
        unsafe {
            let result_ptr = cel_string_last_index_of_offset(
                &string_val as *const CelValue,
                &sub_val as *const CelValue,
                &offset_val as *const CelValue,
            );
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_last_index_of_offset_exceeds_length_returns_error() {
        let string_val = CelValue::String("hello".to_string());
        let sub_val = CelValue::String("l".to_string());
        let offset_val = CelValue::Int(99);
        unsafe {
            let result_ptr = cel_string_last_index_of_offset(
                &string_val as *const CelValue,
                &sub_val as *const CelValue,
                &offset_val as *const CelValue,
            );
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }

    // ── lowerAscii / upperAscii ───────────────────────────────────────────────

    #[rstest]
    #[case::basic("Hello World", "hello world")]
    #[case::already_lower("hello", "hello")]
    #[case::empty("", "")]
    #[case::non_ascii_preserved("Héllo", "héllo")]
    fn test_lower_ascii(#[case] s: &str, #[case] expected: &str) {
        let string_val = CelValue::String(s.to_string());
        unsafe {
            let result_ptr = cel_string_lower_ascii(&string_val as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::String(expected.to_string()));
            cel_free_value(result_ptr);
        }
    }

    #[rstest]
    #[case::basic("hello world", "HELLO WORLD")]
    #[case::already_upper("HELLO", "HELLO")]
    #[case::empty("", "")]
    #[case::non_ascii_preserved("héllo", "HéLLO")]
    fn test_upper_ascii(#[case] s: &str, #[case] expected: &str) {
        let string_val = CelValue::String(s.to_string());
        unsafe {
            let result_ptr = cel_string_upper_ascii(&string_val as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::String(expected.to_string()));
            cel_free_value(result_ptr);
        }
    }

    // ── replace ───────────────────────────────────────────────────────────────

    #[rstest]
    #[case::basic("hello hello", "he", "we", "wello wello")]
    #[case::no_match("hello", "xyz", "abc", "hello")]
    #[case::empty_old("abc", "", "-", "-a-b-c-")]
    fn test_replace(#[case] s: &str, #[case] old: &str, #[case] new: &str, #[case] expected: &str) {
        let string_val = CelValue::String(s.to_string());
        let old_val = CelValue::String(old.to_string());
        let new_val = CelValue::String(new.to_string());
        unsafe {
            let result_ptr = cel_string_replace(
                &string_val as *const CelValue,
                &old_val as *const CelValue,
                &new_val as *const CelValue,
            );
            assert_eq!(&*result_ptr, &CelValue::String(expected.to_string()));
            cel_free_value(result_ptr);
        }
    }

    #[rstest]
    #[case::replace_one("hello hello hello", "hello", "bye", 1, "bye hello hello")]
    #[case::replace_zero("hello hello", "hello", "bye", 0, "hello hello")]
    #[case::replace_all("hello hello", "hello", "bye", 2, "bye bye")]
    fn test_replace_n(
        #[case] s: &str,
        #[case] old: &str,
        #[case] new: &str,
        #[case] n: i64,
        #[case] expected: &str,
    ) {
        let string_val = CelValue::String(s.to_string());
        let old_val = CelValue::String(old.to_string());
        let new_val = CelValue::String(new.to_string());
        let n_val = CelValue::Int(n);
        unsafe {
            let result_ptr = cel_string_replace_n(
                &string_val as *const CelValue,
                &old_val as *const CelValue,
                &new_val as *const CelValue,
                &n_val as *const CelValue,
            );
            assert_eq!(&*result_ptr, &CelValue::String(expected.to_string()));
            cel_free_value(result_ptr);
        }
    }

    // ── split ─────────────────────────────────────────────────────────────────

    #[rstest]
    #[case::basic("a,b,c", ",", vec!["a", "b", "c"])]
    #[case::single_part("abc", "x", vec!["abc"])]
    #[case::empty_string("", ",", vec![""])]
    fn test_split(#[case] s: &str, #[case] sep: &str, #[case] expected: Vec<&str>) {
        let string_val = CelValue::String(s.to_string());
        let sep_val = CelValue::String(sep.to_string());
        let expected_cel: Vec<CelValue> = expected
            .iter()
            .map(|p| CelValue::String(p.to_string()))
            .collect();
        unsafe {
            let result_ptr =
                cel_string_split(&string_val as *const CelValue, &sep_val as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::Array(expected_cel));
            cel_free_value(result_ptr);
        }
    }

    #[rstest]
    #[case::limit_two("a,b,c", ",", 2, vec!["a", "b,c"])]
    #[case::limit_one("a,b,c", ",", 1, vec!["a,b,c"])]
    #[case::unlimited("a,b,c", ",", -1, vec!["a", "b", "c"])]
    fn test_split_n(
        #[case] s: &str,
        #[case] sep: &str,
        #[case] n: i64,
        #[case] expected: Vec<&str>,
    ) {
        let string_val = CelValue::String(s.to_string());
        let sep_val = CelValue::String(sep.to_string());
        let n_val = CelValue::Int(n);
        let expected_cel: Vec<CelValue> = expected
            .iter()
            .map(|p| CelValue::String(p.to_string()))
            .collect();
        unsafe {
            let result_ptr = cel_string_split_n(
                &string_val as *const CelValue,
                &sep_val as *const CelValue,
                &n_val as *const CelValue,
            );
            assert_eq!(&*result_ptr, &CelValue::Array(expected_cel));
            cel_free_value(result_ptr);
        }
    }

    // ── substring ─────────────────────────────────────────────────────────────

    #[rstest]
    #[case::from_offset("hello world", 6, "world")]
    #[case::from_start("hello", 0, "hello")]
    #[case::unicode("café", 2, "fé")]
    fn test_substring(#[case] s: &str, #[case] start: i64, #[case] expected: &str) {
        let string_val = CelValue::String(s.to_string());
        let start_val = CelValue::Int(start);
        unsafe {
            let result_ptr = cel_string_substring(
                &string_val as *const CelValue,
                &start_val as *const CelValue,
            );
            assert_eq!(&*result_ptr, &CelValue::String(expected.to_string()));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_substring_start_out_of_range_returns_error() {
        let string_val = CelValue::String("hello".to_string());
        let start_val = CelValue::Int(99);
        unsafe {
            let result_ptr = cel_string_substring(
                &string_val as *const CelValue,
                &start_val as *const CelValue,
            );
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }

    #[rstest]
    #[case::basic("hello world", 6, 11, "world")]
    #[case::empty_range("hello", 2, 2, "")]
    #[case::full("hello", 0, 5, "hello")]
    fn test_substring_range(
        #[case] s: &str,
        #[case] start: i64,
        #[case] end: i64,
        #[case] expected: &str,
    ) {
        let string_val = CelValue::String(s.to_string());
        let start_val = CelValue::Int(start);
        let end_val = CelValue::Int(end);
        unsafe {
            let result_ptr = cel_string_substring_range(
                &string_val as *const CelValue,
                &start_val as *const CelValue,
                &end_val as *const CelValue,
            );
            assert_eq!(&*result_ptr, &CelValue::String(expected.to_string()));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_substring_range_end_before_start_returns_error() {
        let string_val = CelValue::String("hello".to_string());
        let start_val = CelValue::Int(3);
        let end_val = CelValue::Int(1);
        unsafe {
            let result_ptr = cel_string_substring_range(
                &string_val as *const CelValue,
                &start_val as *const CelValue,
                &end_val as *const CelValue,
            );
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_substring_range_out_of_range_returns_error() {
        let string_val = CelValue::String("hello".to_string());
        let start_val = CelValue::Int(0);
        let end_val = CelValue::Int(99);
        unsafe {
            let result_ptr = cel_string_substring_range(
                &string_val as *const CelValue,
                &start_val as *const CelValue,
                &end_val as *const CelValue,
            );
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }

    // ── trim ──────────────────────────────────────────────────────────────────

    #[rstest]
    #[case::basic("  hello  ", "hello")]
    #[case::no_whitespace("hello", "hello")]
    #[case::empty("", "")]
    #[case::only_whitespace("   ", "")]
    fn test_trim(#[case] s: &str, #[case] expected: &str) {
        let string_val = CelValue::String(s.to_string());
        unsafe {
            let result_ptr = cel_string_trim(&string_val as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::String(expected.to_string()));
            cel_free_value(result_ptr);
        }
    }

    // ── reverse ───────────────────────────────────────────────────────────────

    #[rstest]
    #[case::basic("hello", "olleh")]
    #[case::palindrome("racecar", "racecar")]
    #[case::empty("", "")]
    #[case::unicode("café", "éfac")]
    fn test_reverse(#[case] s: &str, #[case] expected: &str) {
        let string_val = CelValue::String(s.to_string());
        unsafe {
            let result_ptr = cel_string_reverse(&string_val as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::String(expected.to_string()));
            cel_free_value(result_ptr);
        }
    }

    // ── strings.quote ─────────────────────────────────────────────────────────

    #[rstest]
    #[case::plain("hello", r#""hello""#)]
    #[case::with_tab("a\tb", r#""a\tb""#)]
    #[case::with_newline("a\nb", r#""a\nb""#)]
    #[case::with_backslash(r"a\b", r#""a\\b""#)]
    #[case::with_double_quote(r#"a"b"#, r#""a\"b""#)]
    #[case::empty("", r#""""#)]
    fn test_strings_quote(#[case] s: &str, #[case] expected: &str) {
        let string_val = CelValue::String(s.to_string());
        unsafe {
            let result_ptr = cel_strings_quote(&string_val as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::String(expected.to_string()));
            cel_free_value(result_ptr);
        }
    }

    // ── format ────────────────────────────────────────────────────────────────

    #[rstest]
    #[case::string_verb("%s world", vec![CelValue::String("hello".to_string())], "hello world")]
    #[case::int_verb("value: %d", vec![CelValue::Int(42)], "value: 42")]
    #[case::hex_verb("%x", vec![CelValue::Int(255)], "ff")]
    #[case::hex_upper_verb("%X", vec![CelValue::Int(255)], "FF")]
    #[case::percent_escape("100%%", vec![], "100%")]
    fn test_format(#[case] fmt: &str, #[case] args: Vec<CelValue>, #[case] expected: &str) {
        let fmt_val = CelValue::String(fmt.to_string());
        let args_val = CelValue::Array(args);
        unsafe {
            let result_ptr =
                cel_string_format(&fmt_val as *const CelValue, &args_val as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::String(expected.to_string()));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_format_float_with_precision() {
        let fmt_val = CelValue::String("pi: %.2f".to_string());
        let args_val = CelValue::Array(vec![CelValue::Double(3.14159)]);
        unsafe {
            let result_ptr =
                cel_string_format(&fmt_val as *const CelValue, &args_val as *const CelValue);
            assert_eq!(&*result_ptr, &CelValue::String("pi: 3.14".to_string()));
            cel_free_value(result_ptr);
        }
    }

    #[test]
    fn test_format_too_few_args_returns_error() {
        let fmt_val = CelValue::String("%s %s".to_string());
        let args_val = CelValue::Array(vec![CelValue::String("only_one".to_string())]);
        unsafe {
            let result_ptr =
                cel_string_format(&fmt_val as *const CelValue, &args_val as *const CelValue);
            assert!(matches!(&*result_ptr, CelValue::Error(_)));
            cel_free_value(result_ptr);
        }
    }
}
