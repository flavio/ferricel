//! Kubernetes CEL quantity library extensions.
//!
//! Implements the Kubernetes resource quantity functions described in:
//!   <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-quantity-library>
//!
//! Functions:
//!   - `quantity(string)`                   → Quantity (or error if invalid)
//!   - `isQuantity(string)`                 → bool
//!   - `<Q>.sign()`                         → int (-1, 0, or 1)
//!   - `<Q>.isInteger()`                    → bool
//!   - `<Q>.asInteger()`                    → int (or error if not representable)
//!   - `<Q>.asApproximateFloat()`           → double
//!   - `<Q>.add(<Q>)`                       → Quantity
//!   - `<Q>.add(int)`                       → Quantity
//!   - `<Q>.sub(<Q>)`                       → Quantity
//!   - `<Q>.sub(int)`                       → Quantity
//!   - `<Q>.isLessThan(<Q>)`                → bool
//!   - `<Q>.isGreaterThan(<Q>)`             → bool
//!   - `<Q>.compareTo(<Q>)`                 → int (-1, 0, or 1)
//!
//! ## Internal representation
//!
//! Quantities are stored as their canonical string form. On-demand parsing
//! converts to a `QuantityAmount` which stores the value in milli-units (×10³)
//! as an `i128`. This gives exact arithmetic for all practical Kubernetes
//! quantities (memory, CPU, etc.) while handling the various suffix formats.
//!
//! For pathological huge values (e.g. `9999999999999999999999999999999999999G`)
//! that overflow i128 even in milli scale, we use the `Overflow` variant which
//! serialises `asApproximateFloat()` as ±∞ and `asInteger()` as an error.
//!
//! ## Format classes
//!
//! | Class          | Examples                    | Notes                         |
//! |----------------|-----------------------------|-------------------------------|
//! | DecimalSI      | `100`, `100k`, `100M`, `1G` | Decimal (base-10) SI suffixes |
//! | BinarySI       | `100Ki`, `200Mi`, `1Gi`     | Binary (base-2) IEC suffixes  |
//! | DecimalExponent| `1e3`, `1E6`, `1.5e2`       | Decimal exponent notation     |
//!
//! ## Canonicalization
//!
//! After arithmetic, the result is re-serialized to canonical form:
//!   - The receiver's format class is preserved.
//!   - Trailing zeros are stripped.
//!   - The suffix is chosen to keep the coefficient reasonably short.

use crate::error::{create_error_value, read_ptr};
use crate::types::CelValue;
use slog::error;

// ─────────────────────────────────────────────────────────────────────────────
// Format and amount types
// ─────────────────────────────────────────────────────────────────────────────

/// The format class of a Kubernetes quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuantityFormat {
    /// Decimal SI: no suffix, `k`, `M`, `G`, `T`, `P`, `E`
    DecimalSI,
    /// Binary SI (IEC): `Ki`, `Mi`, `Gi`, `Ti`, `Pi`, `Ei`
    BinarySI,
    /// Decimal exponent: `e` or `E` notation
    DecimalExponent,
}

/// Internal arithmetic representation of a quantity.
///
/// Stores the value in milli-units (×10³) as an `i128`. This means:
///   - `1` (unit) is stored as `1000`
///   - `100m` (milli) is stored as `100`
///   - `1k` is stored as `1_000_000`
///   - `1Ki` is stored as `1024 * 1000 = 1_024_000`
///
/// The `Overflow` variant handles values that cannot fit in this range.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum QuantityAmount {
    /// Value fits in milli-unit i128 representation.
    Finite {
        /// Value in milli-units (multiply by 10^-3 to get the actual value).
        milli: i128,
        /// Original format class (for re-serialization).
        format: QuantityFormat,
    },
    /// Value is too large for i128 milli representation.
    Overflow { positive: bool },
}

// ─────────────────────────────────────────────────────────────────────────────
// Parser
// ─────────────────────────────────────────────────────────────────────────────

/// The regex pattern from Go's `k8s.io/apimachinery/pkg/api/resource` (`splitREString`).
/// Used only in error message strings — actual parsing is done by `parse_quantity_full`.
const QUANTITY_REGEX: &str = "^([+-]?[0-9.]+)([eEinumkKMGTP]*[-+]?[0-9]*)$";

/// Internal parse error type for quantity parsing.
#[derive(Debug, thiserror::Error)]
enum ParseError {
    /// The input string is malformed; carries the user-facing error message.
    #[error("{0}")]
    Invalid(String),
    /// The numeric value overflows the i128 milli-unit representation.
    #[error("overflow")]
    Overflow,
}

/// Parse the numeric string and scale to produce a milli-unit value.
/// Returns `Err(ParseError::Overflow)` if the result doesn't fit in i128.
///
/// The computation is: `sign * numeric * scale_num / scale_den * 1000`
///
/// We do this exactly in integer arithmetic to avoid floating-point errors.
fn parse_numeric_to_milli(
    sign: i128,
    numeric_str: &str,
    scale_num: i128,
    scale_den: i128,
) -> Result<i128, ParseError> {
    // Split numeric_str on '.'
    let (int_part, frac_part) = if let Some(dot) = numeric_str.find('.') {
        (&numeric_str[..dot], &numeric_str[dot + 1..])
    } else {
        (numeric_str, "")
    };

    // We need to compute: (int_part * 10^frac_len + frac_part) * scale_num * 1000
    //                    / (10^frac_len * scale_den)
    // This must be exact (no remainder) or we lose sub-milli precision (which is fine—
    // Kubernetes quantities don't go below milli).

    let frac_len = frac_part.len() as u32;

    // Parse integer part
    let int_val: i128 = if int_part.is_empty() {
        0
    } else {
        int_part.parse::<i128>().map_err(|_| {
            ParseError::Invalid(format!(
                "quantities must match the regular expression '{}'",
                QUANTITY_REGEX
            ))
        })?
    };

    // Parse fractional part
    let frac_val: i128 = if frac_part.is_empty() {
        0
    } else {
        frac_part.parse::<i128>().map_err(|_| {
            ParseError::Invalid(format!(
                "quantities must match the regular expression '{}'",
                QUANTITY_REGEX
            ))
        })?
    };

    // 10^frac_len
    let ten_pow_frac = 10i128.checked_pow(frac_len).ok_or(ParseError::Overflow)?;

    // numerator = (int_val * 10^frac_len + frac_val) * scale_num * 1000
    // denominator = 10^frac_len * scale_den
    //
    // To avoid overflow in intermediate steps, we use checked arithmetic and
    // fall back to the Overflow path.
    let combined = int_val
        .checked_mul(ten_pow_frac)
        .and_then(|v| v.checked_add(frac_val));

    let combined = match combined {
        Some(v) => v,
        None => {
            // Value too large for i128; signal overflow to the caller.
            return Err(ParseError::Overflow);
        }
    };

    // numerator = combined * scale_num * 1000
    let num = combined
        .checked_mul(scale_num)
        .and_then(|v| v.checked_mul(1000));
    let den = ten_pow_frac.checked_mul(scale_den);

    match (num, den) {
        (Some(n), Some(d)) => {
            if d == 0 {
                return Err(ParseError::Overflow);
            }
            // Integer division — we truncate sub-milli fractional parts
            // (Kubernetes guarantees quantities are whole milli-units)
            Ok(sign * (n / d))
        }
        _ => Err(ParseError::Overflow),
    }
}

/// Parse a quantity suffix and return `(scale_numerator, scale_denominator, format)`.
///
/// The scale means: `value_in_base_units = numeric * scale_num / scale_den`
/// We then multiply by 1000 for milli-units.
///
/// Returns `None` if the suffix is not recognized.
fn parse_suffix(suffix: &str) -> Option<(i128, i128, QuantityFormat)> {
    match suffix {
        // No suffix: decimal SI, scale = 1
        "" => Some((1, 1, QuantityFormat::DecimalSI)),

        // Decimal SI suffixes (base 10)
        "k" => Some((1_000, 1, QuantityFormat::DecimalSI)),
        "M" => Some((1_000_000, 1, QuantityFormat::DecimalSI)),
        "G" => Some((1_000_000_000, 1, QuantityFormat::DecimalSI)),
        "T" => Some((1_000_000_000_000, 1, QuantityFormat::DecimalSI)),
        "P" => Some((1_000_000_000_000_000, 1, QuantityFormat::DecimalSI)),
        "E" => Some((1_000_000_000_000_000_000, 1, QuantityFormat::DecimalSI)),

        // Milli (sub-unit) decimal SI
        "m" => Some((1, 1000, QuantityFormat::DecimalSI)),

        // Binary SI (IEC) suffixes
        "Ki" => Some((1024, 1, QuantityFormat::BinarySI)),
        "Mi" => Some((1024 * 1024, 1, QuantityFormat::BinarySI)),
        "Gi" => Some((1024 * 1024 * 1024, 1, QuantityFormat::BinarySI)),
        "Ti" => Some((1024i128 * 1024 * 1024 * 1024, 1, QuantityFormat::BinarySI)),
        "Pi" => Some((
            1024i128 * 1024 * 1024 * 1024 * 1024,
            1,
            QuantityFormat::BinarySI,
        )),
        "Ei" => Some((
            1024i128 * 1024 * 1024 * 1024 * 1024 * 1024,
            1,
            QuantityFormat::BinarySI,
        )),

        // Decimal exponent notation: eN or EN
        _ if suffix.starts_with('e') || suffix.starts_with('E') => {
            let exp_str = &suffix[1..];
            let exp: i32 = exp_str.parse().ok()?;
            if exp >= 0 {
                let exp = exp as u32;
                // scale = 10^exp
                let scale = 10i128.checked_pow(exp)?;
                Some((scale, 1, QuantityFormat::DecimalExponent))
            } else {
                let exp = (-exp) as u32;
                let scale = 10i128.checked_pow(exp)?;
                Some((1, scale, QuantityFormat::DecimalExponent))
            }
        }

        _ => None,
    }
}

/// Parse a quantity string into a `QuantityAmount`.
///
/// Returns `Ok(QuantityAmount::Overflow)` for values that exceed the i128
/// milli-unit range, rather than an error, so callers can still perform
/// best-effort operations (e.g. `asApproximateFloat` returns ±∞).
fn parse_quantity_full(s: &str) -> Result<QuantityAmount, String> {
    // Track sign upfront so we can set the correct Overflow polarity.
    let positive = !s.starts_with('-');

    if s.is_empty() {
        return Err(format!(
            "quantities must match the regular expression '{}'",
            QUANTITY_REGEX
        ));
    }

    let (sign, rest) = if let Some(r) = s.strip_prefix('-') {
        (-1i128, r)
    } else if let Some(r) = s.strip_prefix('+') {
        (1i128, r)
    } else {
        (1i128, s)
    };

    let suffix_start = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(rest.len());
    let numeric_str = &rest[..suffix_start];
    let suffix = &rest[suffix_start..];

    if numeric_str.is_empty() {
        return Err(format!(
            "quantities must match the regular expression '{}'",
            QUANTITY_REGEX
        ));
    }

    let dot_count = numeric_str.chars().filter(|&c| c == '.').count();
    if dot_count > 1 {
        return Err(format!(
            "quantities must match the regular expression '{}'",
            QUANTITY_REGEX
        ));
    }

    match parse_suffix(suffix) {
        None => Err("unable to parse quantity's suffix".to_string()),
        Some((scale_num, scale_den, format)) => {
            match parse_numeric_to_milli(sign, numeric_str, scale_num, scale_den) {
                Ok(milli) => Ok(QuantityAmount::Finite { milli, format }),
                Err(ParseError::Overflow) => Ok(QuantityAmount::Overflow { positive }),
                Err(ParseError::Invalid(msg)) => Err(msg),
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Canonicalization (serialize back to string)
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a `QuantityAmount` back to its canonical string form.
///
/// For `Finite` values we pick the "best" suffix for the given format class:
///   - The largest suffix that results in a coefficient ≥ 1.
///   - If `milli == 0` we return `"0"`.
pub(crate) fn canonicalize(amount: &QuantityAmount) -> String {
    match amount {
        QuantityAmount::Overflow { positive } => {
            if *positive {
                "9999999999999999999999999999999999999G".to_string()
            } else {
                "-9999999999999999999999999999999999999G".to_string()
            }
        }
        QuantityAmount::Finite { milli, format } => {
            if *milli == 0 {
                return "0".to_string();
            }
            let sign = if *milli < 0 { "-" } else { "" };
            let abs_milli = milli.unsigned_abs(); // u128

            match format {
                QuantityFormat::DecimalSI => canonicalize_decimal_si(sign, abs_milli),
                QuantityFormat::BinarySI => canonicalize_binary_si(sign, abs_milli),
                QuantityFormat::DecimalExponent => canonicalize_decimal_exp(sign, abs_milli),
            }
        }
    }
}

/// Scale table for decimal SI (in milli-units): (suffix, milli_value)
const DECIMAL_SI_SCALES: &[(&str, u128)] = &[
    ("E", 1_000_000_000_000_000_000_000u128), // 10^18 * 1000
    ("P", 1_000_000_000_000_000_000u128),     // 10^15 * 1000
    ("T", 1_000_000_000_000_000u128),         // 10^12 * 1000
    ("G", 1_000_000_000_000u128),             // 10^9 * 1000
    ("M", 1_000_000_000u128),                 // 10^6 * 1000
    ("k", 1_000_000u128),                     // 10^3 * 1000
    ("", 1_000u128),                          // 10^0 * 1000 (units)
    ("m", 1u128),                             // 10^-3 * 1000 (milli)
];

fn canonicalize_decimal_si(sign: &str, abs_milli: u128) -> String {
    for (suffix, scale) in DECIMAL_SI_SCALES {
        if abs_milli >= *scale && abs_milli.is_multiple_of(*scale) {
            let coeff = abs_milli / scale;
            return format!("{}{}{}", sign, coeff, suffix);
        }
    }
    // sub-milli: shouldn't happen for well-formed quantities, but just in case
    format!("{}{}m", sign, abs_milli)
}

/// Scale table for binary SI (in milli-units)
const BINARY_SI_SCALES: &[(&str, u128)] = &[
    ("Ei", 1024u128 * 1024 * 1024 * 1024 * 1024 * 1024 * 1000),
    ("Pi", 1024u128 * 1024 * 1024 * 1024 * 1024 * 1000),
    ("Ti", 1024u128 * 1024 * 1024 * 1024 * 1000),
    ("Gi", 1024u128 * 1024 * 1024 * 1000),
    ("Mi", 1024u128 * 1024 * 1000),
    ("Ki", 1024u128 * 1000),
    ("", 1_000u128), // plain units
];

fn canonicalize_binary_si(sign: &str, abs_milli: u128) -> String {
    for (suffix, scale) in BINARY_SI_SCALES {
        if abs_milli >= *scale && abs_milli.is_multiple_of(*scale) {
            let coeff = abs_milli / scale;
            return format!("{}{}{}", sign, coeff, suffix);
        }
    }
    // Fall back to milli (decimal)
    format!("{}{}m", sign, abs_milli)
}

fn canonicalize_decimal_exp(sign: &str, abs_milli: u128) -> String {
    // For decimal exponent format, use the Go behavior:
    // express as Xe<N> where X is an integer and N is chosen to keep X small.
    // But for simplicity we output in the same style as DecimalSI but with 'e' notation
    // when the exponent is non-zero.
    //
    // Actually Go's resource package always uses DecimalSI canonicalization when the
    // value can be expressed exactly with a standard suffix. DecimalExponent is only
    // used when the original input used 'e' notation.
    //
    // We'll follow the same approach: find the best (exponent, coefficient) pair.
    if abs_milli == 0 {
        return "0".to_string();
    }

    // Try to express abs_milli as coeff * 10^exp * 1000 (since abs_milli is in milli-units)
    // i.e. actual value = coeff * 10^exp
    // Find largest exp such that coeff is integer
    let mut coeff = abs_milli;
    let mut exp: i32 = -3; // because abs_milli is in milli-units (×10^-3)
    while coeff.is_multiple_of(10) {
        coeff /= 10;
        exp += 1;
    }
    // Now value = coeff * 10^exp (where coeff has no trailing zeros)
    if exp == 0 {
        format!("{}{}", sign, coeff)
    } else if exp > 0 {
        format!("{}{}e{}", sign, coeff, exp)
    } else {
        // Negative exponent: express as decimal
        // e.g. coeff=15, exp=-1 → 1.5
        // For simplicity, use milli if fractional
        // exp == -3 means milli
        if exp == -3 {
            format!("{}{}m", sign, coeff)
        } else {
            // e.g. coeff * 10^-1: write as decimal
            let exp_abs = (-exp) as u32;
            let divisor = 10u128.pow(exp_abs);
            let int_part = coeff / divisor;
            let frac_part = coeff % divisor;
            if frac_part == 0 {
                format!("{}{}", sign, int_part)
            } else {
                format!(
                    "{}{}.{:0>width$}",
                    sign,
                    int_part,
                    frac_part,
                    width = exp_abs as usize
                )
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Arithmetic helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Add two `QuantityAmount` values. The result uses the receiver's format.
fn quantity_add(lhs: &QuantityAmount, rhs_milli: i128) -> QuantityAmount {
    match lhs {
        QuantityAmount::Overflow { positive } => QuantityAmount::Overflow {
            positive: *positive,
        },
        QuantityAmount::Finite { milli, format } => match milli.checked_add(rhs_milli) {
            Some(result) => QuantityAmount::Finite {
                milli: result,
                format: *format,
            },
            None => QuantityAmount::Overflow {
                positive: *milli > 0,
            },
        },
    }
}

/// Get the milli value from a `QuantityAmount`, or 0 for overflow (best-effort).
#[cfg(test)]
fn milli_of(amount: &QuantityAmount) -> Option<i128> {
    match amount {
        QuantityAmount::Finite { milli, .. } => Some(*milli),
        QuantityAmount::Overflow { .. } => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// isInteger helper
// ─────────────────────────────────────────────────────────────────────────────

/// Returns true if the quantity represents a whole number (no fractional milli-units).
/// A value is integer if its milli value is divisible by 1000.
fn is_integer_amount(amount: &QuantityAmount) -> bool {
    match amount {
        QuantityAmount::Finite { milli, .. } => milli % 1000 == 0,
        QuantityAmount::Overflow { .. } => false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Equality helper (called from helpers.rs)
// ─────────────────────────────────────────────────────────────────────────────

/// Compare two quantity strings for equality by numeric value.
pub(crate) fn quantities_equal(a: &str, b: &str) -> bool {
    match (parse_quantity_full(a), parse_quantity_full(b)) {
        (
            Ok(QuantityAmount::Finite { milli: a, .. }),
            Ok(QuantityAmount::Finite { milli: b, .. }),
        ) => a == b,
        (
            Ok(QuantityAmount::Overflow { positive: a }),
            Ok(QuantityAmount::Overflow { positive: b }),
        ) => a == b,
        _ => false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WASM-callable runtime functions
// ─────────────────────────────────────────────────────────────────────────────

/// `quantity(string)` → Quantity or Error
///
/// # Safety
/// `str_ptr` must be a valid, non-null pointer to a `CelValue::String`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_quantity_parse(str_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if str_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_quantity_parse");
        return create_error_value("no such overload");
    }

    let val = unsafe { read_ptr(str_ptr) };
    let s = match val {
        CelValue::String(s) => s,
        other => {
            error!(log, "expected String"; "function" => "cel_k8s_quantity_parse", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    match parse_quantity_full(&s) {
        Ok(_) => Box::into_raw(Box::new(CelValue::Quantity(s))),
        Err(msg) => {
            error!(log, "invalid quantity"; "function" => "cel_k8s_quantity_parse", "error" => &msg);
            create_error_value(&msg)
        }
    }
}

/// `isQuantity(string)` → bool
///
/// # Safety
/// `str_ptr` must be a valid, non-null pointer to a `CelValue::String`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_is_quantity(str_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if str_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_is_quantity");
        return create_error_value("no such overload");
    }

    let val = unsafe { read_ptr(str_ptr) };
    let s = match val {
        CelValue::String(s) => s,
        other => {
            error!(log, "expected String"; "function" => "cel_k8s_is_quantity", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let ok = parse_quantity_full(&s).is_ok();
    Box::into_raw(Box::new(CelValue::Bool(ok)))
}

/// Helper to parse a Quantity CelValue into a QuantityAmount, returning an error CelValue on failure.
///
unsafe fn parse_quantity_cel(
    ptr: *mut CelValue,
    fn_name: &str,
) -> Result<QuantityAmount, *mut CelValue> {
    let log = crate::logging::get_logger();
    if ptr.is_null() {
        error!(log, "null pointer"; "function" => fn_name);
        return Err(create_error_value("no such overload"));
    }
    let val = unsafe { read_ptr(ptr) };
    match val {
        CelValue::Quantity(s) => parse_quantity_full(&s).map_err(|e| {
            error!(log, "invalid quantity"; "function" => fn_name, "error" => &e);
            create_error_value(&e)
        }),
        other => {
            error!(log, "expected Quantity"; "function" => fn_name, "got" => format!("{:?}", other));
            Err(create_error_value("no such overload"))
        }
    }
}

/// `<Q>.sign()` → int (-1, 0, or 1)
///
/// # Safety
/// `q_ptr` must be a valid, non-null pointer to a `CelValue::Quantity`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_quantity_sign(q_ptr: *mut CelValue) -> *mut CelValue {
    match unsafe { parse_quantity_cel(q_ptr, "cel_k8s_quantity_sign") } {
        Err(e) => e,
        Ok(amount) => {
            let result: i64 = match amount {
                QuantityAmount::Overflow { positive } => {
                    if positive {
                        1
                    } else {
                        -1
                    }
                }
                QuantityAmount::Finite { milli, .. } => {
                    if milli > 0 {
                        1
                    } else if milli < 0 {
                        -1
                    } else {
                        0
                    }
                }
            };
            Box::into_raw(Box::new(CelValue::Int(result)))
        }
    }
}

/// `<Q>.isInteger()` → bool
///
/// # Safety
/// `q_ptr` must be a valid, non-null pointer to a `CelValue::Quantity`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_quantity_is_integer(q_ptr: *mut CelValue) -> *mut CelValue {
    match unsafe { parse_quantity_cel(q_ptr, "cel_k8s_quantity_is_integer") } {
        Err(e) => e,
        Ok(amount) => Box::into_raw(Box::new(CelValue::Bool(is_integer_amount(&amount)))),
    }
}

/// `<Q>.asInteger()` → int or Error
///
/// # Safety
/// `q_ptr` must be a valid, non-null pointer to a `CelValue::Quantity`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_quantity_as_integer(q_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();
    match unsafe { parse_quantity_cel(q_ptr, "cel_k8s_quantity_as_integer") } {
        Err(e) => e,
        Ok(amount) => match amount {
            QuantityAmount::Overflow { .. } => {
                error!(log, "overflow in asInteger"; "function" => "cel_k8s_quantity_as_integer");
                create_error_value("cannot convert value to integer")
            }
            QuantityAmount::Finite { milli, .. } => {
                if milli % 1000 != 0 {
                    error!(log, "fractional value in asInteger"; "function" => "cel_k8s_quantity_as_integer");
                    return create_error_value("cannot convert value to integer");
                }
                let int_val = milli / 1000;
                // Check it fits in i64
                if int_val < i64::MIN as i128 || int_val > i64::MAX as i128 {
                    error!(log, "value out of i64 range"; "function" => "cel_k8s_quantity_as_integer");
                    return create_error_value("cannot convert value to integer");
                }
                Box::into_raw(Box::new(CelValue::Int(int_val as i64)))
            }
        },
    }
}

/// `<Q>.asApproximateFloat()` → double
///
/// # Safety
/// `q_ptr` must be a valid, non-null pointer to a `CelValue::Quantity`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_quantity_as_approx_float(q_ptr: *mut CelValue) -> *mut CelValue {
    match unsafe { parse_quantity_cel(q_ptr, "cel_k8s_quantity_as_approx_float") } {
        Err(e) => e,
        Ok(amount) => {
            let f: f64 = match amount {
                QuantityAmount::Overflow { positive } => {
                    if positive {
                        f64::INFINITY
                    } else {
                        f64::NEG_INFINITY
                    }
                }
                QuantityAmount::Finite { milli, .. } => (milli as f64) / 1000.0,
            };
            Box::into_raw(Box::new(CelValue::Double(f)))
        }
    }
}

/// `<Q>.add(<Q>)` → Quantity
///
/// # Safety
/// Both pointers must be valid, non-null pointers to `CelValue::Quantity`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_quantity_add(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    let lhs = match unsafe { parse_quantity_cel(lhs_ptr, "cel_k8s_quantity_add") } {
        Err(e) => return e,
        Ok(a) => a,
    };
    let rhs = match unsafe { parse_quantity_cel(rhs_ptr, "cel_k8s_quantity_add") } {
        Err(e) => return e,
        Ok(a) => a,
    };

    let rhs_milli = match rhs {
        QuantityAmount::Finite { milli, .. } => milli,
        QuantityAmount::Overflow { positive } => {
            return Box::into_raw(Box::new(CelValue::Quantity(canonicalize(
                &QuantityAmount::Overflow { positive },
            ))));
        }
    };

    let result = quantity_add(&lhs, rhs_milli);
    Box::into_raw(Box::new(CelValue::Quantity(canonicalize(&result))))
}

/// `<Q>.add(int)` → Quantity
///
/// The integer is in raw units (not milli), so `quantity("50k").add(20) == quantity("50020")`.
///
/// # Safety
/// `q_ptr` must be a valid, non-null pointer to a `CelValue::Quantity`.
/// `int_ptr` must be a valid, non-null pointer to a `CelValue::Int`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_quantity_add_int(
    q_ptr: *mut CelValue,
    int_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    let lhs = match unsafe { parse_quantity_cel(q_ptr, "cel_k8s_quantity_add_int") } {
        Err(e) => return e,
        Ok(a) => a,
    };

    if int_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_quantity_add_int");
        return create_error_value("no such overload");
    }
    let int_val = unsafe { read_ptr(int_ptr) };
    let n = match int_val {
        CelValue::Int(n) => n,
        other => {
            error!(log, "expected Int"; "function" => "cel_k8s_quantity_add_int", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    // Convert int (in units) to milli
    let rhs_milli = match (n as i128).checked_mul(1000) {
        Some(v) => v,
        None => {
            // Overflow
            let result = QuantityAmount::Overflow { positive: n >= 0 };
            return Box::into_raw(Box::new(CelValue::Quantity(canonicalize(&result))));
        }
    };

    let result = quantity_add(&lhs, rhs_milli);
    Box::into_raw(Box::new(CelValue::Quantity(canonicalize(&result))))
}

/// `<Q>.sub(<Q>)` → Quantity
///
/// # Safety
/// Both pointers must be valid, non-null pointers to `CelValue::Quantity`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_quantity_sub(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    let lhs = match unsafe { parse_quantity_cel(lhs_ptr, "cel_k8s_quantity_sub") } {
        Err(e) => return e,
        Ok(a) => a,
    };
    let rhs = match unsafe { parse_quantity_cel(rhs_ptr, "cel_k8s_quantity_sub") } {
        Err(e) => return e,
        Ok(a) => a,
    };

    let rhs_milli = match rhs {
        QuantityAmount::Finite { milli, .. } => milli,
        QuantityAmount::Overflow { positive } => {
            return Box::into_raw(Box::new(CelValue::Quantity(canonicalize(
                &QuantityAmount::Overflow {
                    positive: !positive,
                },
            ))));
        }
    };

    let result = quantity_add(&lhs, -rhs_milli);
    Box::into_raw(Box::new(CelValue::Quantity(canonicalize(&result))))
}

/// `<Q>.sub(int)` → Quantity
///
/// # Safety
/// `q_ptr` must be a valid, non-null pointer to a `CelValue::Quantity`.
/// `int_ptr` must be a valid, non-null pointer to a `CelValue::Int`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_quantity_sub_int(
    q_ptr: *mut CelValue,
    int_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    let lhs = match unsafe { parse_quantity_cel(q_ptr, "cel_k8s_quantity_sub_int") } {
        Err(e) => return e,
        Ok(a) => a,
    };

    if int_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_quantity_sub_int");
        return create_error_value("no such overload");
    }
    let int_val = unsafe { read_ptr(int_ptr) };
    let n = match int_val {
        CelValue::Int(n) => n,
        other => {
            error!(log, "expected Int"; "function" => "cel_k8s_quantity_sub_int", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let rhs_milli = match (n as i128).checked_mul(1000) {
        Some(v) => v,
        None => {
            let result = QuantityAmount::Overflow { positive: n < 0 }; // subtracting negative overflow = positive overflow
            return Box::into_raw(Box::new(CelValue::Quantity(canonicalize(&result))));
        }
    };

    let result = quantity_add(&lhs, -rhs_milli);
    Box::into_raw(Box::new(CelValue::Quantity(canonicalize(&result))))
}

/// Compare two Quantity CelValues, returning an `Ordering`.
unsafe fn compare_quantities(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
    fn_name: &str,
) -> Result<std::cmp::Ordering, *mut CelValue> {
    let lhs = unsafe { parse_quantity_cel(lhs_ptr, fn_name) }?;
    let rhs = unsafe { parse_quantity_cel(rhs_ptr, fn_name) }?;

    let ord = match (&lhs, &rhs) {
        (QuantityAmount::Finite { milli: a, .. }, QuantityAmount::Finite { milli: b, .. }) => {
            a.cmp(b)
        }
        (QuantityAmount::Overflow { positive: a }, QuantityAmount::Overflow { positive: b }) => {
            a.cmp(b) // false < true ⇒ negative overflow < positive overflow
        }
        (QuantityAmount::Overflow { positive: true }, QuantityAmount::Finite { .. }) => {
            std::cmp::Ordering::Greater
        }
        (QuantityAmount::Overflow { positive: false }, QuantityAmount::Finite { .. }) => {
            std::cmp::Ordering::Less
        }
        (QuantityAmount::Finite { .. }, QuantityAmount::Overflow { positive: true }) => {
            std::cmp::Ordering::Less
        }
        (QuantityAmount::Finite { .. }, QuantityAmount::Overflow { positive: false }) => {
            std::cmp::Ordering::Greater
        }
    };
    Ok(ord)
}

/// `<Q>.isLessThan(<Q>)` — internal helper called by poly dispatch.
///
/// # Safety
/// Both pointers must be valid, non-null pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn cel_k8s_quantity_is_less_than(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    match unsafe { compare_quantities(lhs_ptr, rhs_ptr, "cel_k8s_quantity_is_less_than") } {
        Ok(ord) => Box::into_raw(Box::new(CelValue::Bool(ord == std::cmp::Ordering::Less))),
        Err(e) => e,
    }
}

/// `<Q>.isGreaterThan(<Q>)` — internal helper called by poly dispatch.
///
/// # Safety
/// Both pointers must be valid, non-null pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn cel_k8s_quantity_is_greater_than(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    match unsafe { compare_quantities(lhs_ptr, rhs_ptr, "cel_k8s_quantity_is_greater_than") } {
        Ok(ord) => Box::into_raw(Box::new(CelValue::Bool(ord == std::cmp::Ordering::Greater))),
        Err(e) => e,
    }
}

/// `<Q>.compareTo(<Q>)` — internal helper called by poly dispatch.
///
/// # Safety
/// Both pointers must be valid, non-null pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn cel_k8s_quantity_compare_to(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    match unsafe { compare_quantities(lhs_ptr, rhs_ptr, "cel_k8s_quantity_compare_to") } {
        Ok(ord) => {
            let n: i64 = match ord {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            Box::into_raw(Box::new(CelValue::Int(n)))
        }
        Err(e) => e,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{make_int, make_str, read_val};
    use super::*;
    use rstest::rstest;

    unsafe fn make_quantity(s: &str) -> *mut CelValue {
        let str_ptr = make_str(s);
        unsafe { cel_k8s_quantity_parse(str_ptr) }
    }

    // ── parse_quantity_full ────────────────────────────────────────────────

    #[rstest]
    #[case("0", QuantityAmount::Finite { milli: 0, format: QuantityFormat::DecimalSI })]
    #[case("1", QuantityAmount::Finite { milli: 1000, format: QuantityFormat::DecimalSI })]
    #[case("1k", QuantityAmount::Finite { milli: 1_000_000, format: QuantityFormat::DecimalSI })]
    #[case("1M", QuantityAmount::Finite { milli: 1_000_000_000, format: QuantityFormat::DecimalSI })]
    #[case("100m", QuantityAmount::Finite { milli: 100, format: QuantityFormat::DecimalSI })]
    #[case("1Ki", QuantityAmount::Finite { milli: 1024 * 1000, format: QuantityFormat::BinarySI })]
    #[case("1Mi", QuantityAmount::Finite { milli: 1024 * 1024 * 1000, format: QuantityFormat::BinarySI })]
    #[case("1e3", QuantityAmount::Finite { milli: 1_000_000, format: QuantityFormat::DecimalExponent })]
    #[case("1.5", QuantityAmount::Finite { milli: 1500, format: QuantityFormat::DecimalSI })]
    #[case("50k", QuantityAmount::Finite { milli: 50_000_000, format: QuantityFormat::DecimalSI })]
    fn test_parse_quantity(#[case] input: &str, #[case] expected: QuantityAmount) {
        let result = parse_quantity_full(input).expect("should parse");
        assert_eq!(result, expected, "parse_quantity_full({:?})", input);
    }

    #[rstest]
    #[case("")]
    #[case("abc")]
    #[case("1invalid")]
    fn test_parse_quantity_invalid(#[case] input: &str) {
        assert!(
            parse_quantity_full(input).is_err(),
            "expected error for {:?}",
            input
        );
    }

    // ── canonicalize ──────────────────────────────────────────────────────

    #[rstest]
    #[case(QuantityAmount::Finite { milli: 0, format: QuantityFormat::DecimalSI }, "0")]
    #[case(QuantityAmount::Finite { milli: 1000, format: QuantityFormat::DecimalSI }, "1")]
    #[case(QuantityAmount::Finite { milli: 1_000_000, format: QuantityFormat::DecimalSI }, "1k")]
    #[case(QuantityAmount::Finite { milli: 100, format: QuantityFormat::DecimalSI }, "100m")]
    #[case(QuantityAmount::Finite { milli: 1024 * 1000, format: QuantityFormat::BinarySI }, "1Ki")]
    fn test_canonicalize(#[case] amount: QuantityAmount, #[case] expected: &str) {
        assert_eq!(canonicalize(&amount), expected);
    }

    // ── isQuantity ────────────────────────────────────────────────────────

    #[rstest]
    #[case("0", true)]
    #[case("100m", true)]
    #[case("1Ki", true)]
    #[case("1.5", true)]
    #[case("1e3", true)]
    #[case("", false)]
    #[case("abc", false)]
    fn test_is_quantity(#[case] input: &str, #[case] expected: bool) {
        let str_ptr = make_str(input);
        let result = unsafe { read_val(cel_k8s_is_quantity(str_ptr)) };
        assert_eq!(result, CelValue::Bool(expected), "isQuantity({:?})", input);
    }

    // ── sign ──────────────────────────────────────────────────────────────

    #[rstest]
    #[case("1", 1i64)]
    #[case("0", 0i64)]
    #[case("-1", -1i64)]
    #[case("100m", 1i64)]
    #[case("-100m", -1i64)]
    fn test_sign(#[case] input: &str, #[case] expected: i64) {
        let q_ptr = unsafe { make_quantity(input) };
        let result = unsafe { read_val(cel_k8s_quantity_sign(q_ptr)) };
        assert_eq!(result, CelValue::Int(expected), "sign({:?})", input);
    }

    // ── isInteger ─────────────────────────────────────────────────────────

    #[rstest]
    #[case("1", true)]
    #[case("1k", true)]
    #[case("1Ki", true)]
    #[case("100m", false)]
    #[case("1.5", false)]
    fn test_is_integer(#[case] input: &str, #[case] expected: bool) {
        let q_ptr = unsafe { make_quantity(input) };
        let result = unsafe { read_val(cel_k8s_quantity_is_integer(q_ptr)) };
        assert_eq!(result, CelValue::Bool(expected), "isInteger({:?})", input);
    }

    // ── asInteger ─────────────────────────────────────────────────────────

    #[rstest]
    #[case("1", 1i64)]
    #[case("1k", 1000i64)]
    #[case("2Ki", 2048i64)]
    fn test_as_integer_ok(#[case] input: &str, #[case] expected: i64) {
        let q_ptr = unsafe { make_quantity(input) };
        let result = unsafe { read_val(cel_k8s_quantity_as_integer(q_ptr)) };
        assert_eq!(result, CelValue::Int(expected), "asInteger({:?})", input);
    }

    #[rstest]
    #[case("100m")] // fractional
    #[case("1.5")] // fractional
    fn test_as_integer_error(#[case] input: &str) {
        let q_ptr = unsafe { make_quantity(input) };
        let result = unsafe { read_val(cel_k8s_quantity_as_integer(q_ptr)) };
        assert!(
            matches!(result, CelValue::Error(_)),
            "asInteger({:?}) should be Error",
            input
        );
    }

    // ── asApproximateFloat ────────────────────────────────────────────────

    #[rstest]
    #[case("1", 1.0f64)]
    #[case("1k", 1000.0f64)]
    #[case("100m", 0.1f64)]
    #[case("1.5", 1.5f64)]
    fn test_as_approx_float(#[case] input: &str, #[case] expected: f64) {
        let q_ptr = unsafe { make_quantity(input) };
        let result = unsafe { read_val(cel_k8s_quantity_as_approx_float(q_ptr)) };
        match result {
            CelValue::Double(d) => {
                assert!(
                    (d - expected).abs() < 1e-9,
                    "asApproximateFloat({:?}) = {} (expected {})",
                    input,
                    d,
                    expected
                );
            }
            other => panic!("expected Double, got {:?}", other),
        }
    }

    // ── add ───────────────────────────────────────────────────────────────

    #[test]
    fn test_add_quantities() {
        // quantity("50k").add(quantity("50k")) == quantity("100k")
        let lhs = unsafe { make_quantity("50k") };
        let rhs = unsafe { make_quantity("50k") };
        let result = unsafe { read_val(cel_k8s_quantity_add(lhs, rhs)) };
        match result {
            CelValue::Quantity(s) => {
                let amt = parse_quantity_full(&s).unwrap();
                let expected = parse_quantity_full("100k").unwrap();
                assert_eq!(milli_of(&amt), milli_of(&expected), "50k + 50k = 100k");
            }
            other => panic!("expected Quantity, got {:?}", other),
        }
    }

    #[test]
    fn test_add_int() {
        // quantity("50k").add(20) == quantity("50020")
        let q = unsafe { make_quantity("50k") };
        let n = make_int(20);
        let result = unsafe { read_val(cel_k8s_quantity_add_int(q, n)) };
        match result {
            CelValue::Quantity(s) => {
                let amt = parse_quantity_full(&s).unwrap();
                let expected = parse_quantity_full("50020").unwrap();
                assert_eq!(milli_of(&amt), milli_of(&expected), "50k + 20 = 50020");
            }
            other => panic!("expected Quantity, got {:?}", other),
        }
    }

    // ── sub ───────────────────────────────────────────────────────────────

    #[test]
    fn test_sub_quantities() {
        // quantity("100k").sub(quantity("50k")) == quantity("50k")
        let lhs = unsafe { make_quantity("100k") };
        let rhs = unsafe { make_quantity("50k") };
        let result = unsafe { read_val(cel_k8s_quantity_sub(lhs, rhs)) };
        match result {
            CelValue::Quantity(s) => {
                let amt = parse_quantity_full(&s).unwrap();
                let expected = parse_quantity_full("50k").unwrap();
                assert_eq!(milli_of(&amt), milli_of(&expected), "100k - 50k = 50k");
            }
            other => panic!("expected Quantity, got {:?}", other),
        }
    }

    #[test]
    fn test_sub_int() {
        let q = unsafe { make_quantity("50k") };
        let n = make_int(20);
        let result = unsafe { read_val(cel_k8s_quantity_sub_int(q, n)) };
        match result {
            CelValue::Quantity(s) => {
                let amt = parse_quantity_full(&s).unwrap();
                let expected = parse_quantity_full("49980").unwrap();
                assert_eq!(milli_of(&amt), milli_of(&expected), "50k - 20 = 49980");
            }
            other => panic!("expected Quantity, got {:?}", other),
        }
    }

    // ── equality via quantities_equal ─────────────────────────────────────

    #[rstest]
    #[case("200M", "0.2G", true)]
    #[case("1Ki", "1024", true)]
    #[case("1k", "1000", true)]
    #[case("1k", "2k", false)]
    fn test_quantities_equal(#[case] a: &str, #[case] b: &str, #[case] expected: bool) {
        assert_eq!(quantities_equal(a, b), expected, "{}=={}", a, b);
    }

    // ── overflow ──────────────────────────────────────────────────────────

    const OVERFLOW_POS: &str = "9999999999999999999999999999999999999G";
    const OVERFLOW_NEG: &str = "-9999999999999999999999999999999999999G";

    #[rstest]
    #[case(OVERFLOW_POS, QuantityAmount::Overflow { positive: true })]
    #[case(OVERFLOW_NEG, QuantityAmount::Overflow { positive: false })]
    fn test_parse_overflow(#[case] input: &str, #[case] expected: QuantityAmount) {
        let result = parse_quantity_full(input).expect("should parse as overflow, not error");
        assert_eq!(result, expected, "parse_quantity_full({:?})", input);
    }

    #[rstest]
    #[case(QuantityAmount::Overflow { positive: true },  OVERFLOW_POS)]
    #[case(QuantityAmount::Overflow { positive: false }, OVERFLOW_NEG)]
    fn test_canonicalize_overflow(#[case] amount: QuantityAmount, #[case] expected: &str) {
        assert_eq!(canonicalize(&amount), expected);
    }

    #[rstest]
    #[case(OVERFLOW_POS, 1i64)]
    #[case(OVERFLOW_NEG, -1i64)]
    fn test_sign_overflow(#[case] input: &str, #[case] expected: i64) {
        let q_ptr = unsafe { make_quantity(input) };
        let result = unsafe { read_val(cel_k8s_quantity_sign(q_ptr)) };
        assert_eq!(result, CelValue::Int(expected), "sign({:?})", input);
    }

    #[test]
    fn test_is_integer_overflow() {
        let q_ptr = unsafe { make_quantity(OVERFLOW_POS) };
        let result = unsafe { read_val(cel_k8s_quantity_is_integer(q_ptr)) };
        assert_eq!(
            result,
            CelValue::Bool(false),
            "isInteger(overflow) should be false"
        );
    }

    #[rstest]
    #[case(OVERFLOW_POS)]
    #[case(OVERFLOW_NEG)]
    fn test_as_integer_overflow_error(#[case] input: &str) {
        let q_ptr = unsafe { make_quantity(input) };
        let result = unsafe { read_val(cel_k8s_quantity_as_integer(q_ptr)) };
        assert!(
            matches!(result, CelValue::Error(_)),
            "asInteger({:?}) should be Error",
            input
        );
    }

    #[rstest]
    #[case(OVERFLOW_POS, f64::INFINITY)]
    #[case(OVERFLOW_NEG, f64::NEG_INFINITY)]
    fn test_as_approx_float_overflow(#[case] input: &str, #[case] expected: f64) {
        let q_ptr = unsafe { make_quantity(input) };
        let result = unsafe { read_val(cel_k8s_quantity_as_approx_float(q_ptr)) };
        match result {
            CelValue::Double(d) => assert_eq!(d, expected, "asApproximateFloat({:?})", input),
            other => panic!("expected Double, got {:?}", other),
        }
    }

    #[test]
    fn test_add_overflow_operand() {
        // overflow + finite → overflow
        let lhs = unsafe { make_quantity(OVERFLOW_POS) };
        let rhs = unsafe { make_quantity("1k") };
        let result = unsafe { read_val(cel_k8s_quantity_add(lhs, rhs)) };
        match result {
            CelValue::Quantity(s) => assert_eq!(
                parse_quantity_full(&s).unwrap(),
                QuantityAmount::Overflow { positive: true },
                "overflow + 1k should stay overflow"
            ),
            other => panic!("expected Quantity, got {:?}", other),
        }

        // finite + overflow → overflow
        let lhs = unsafe { make_quantity("1k") };
        let rhs = unsafe { make_quantity(OVERFLOW_POS) };
        let result = unsafe { read_val(cel_k8s_quantity_add(lhs, rhs)) };
        match result {
            CelValue::Quantity(s) => assert_eq!(
                parse_quantity_full(&s).unwrap(),
                QuantityAmount::Overflow { positive: true },
                "1k + overflow should be overflow"
            ),
            other => panic!("expected Quantity, got {:?}", other),
        }
    }

    #[test]
    fn test_add_int_overflow_result() {
        // A very large finite quantity + large int overflows at the arithmetic level.
        // Use the overflow sentinel directly as lhs.
        let q = unsafe { make_quantity(OVERFLOW_POS) };
        let n = make_int(1);
        let result = unsafe { read_val(cel_k8s_quantity_add_int(q, n)) };
        match result {
            CelValue::Quantity(s) => assert_eq!(
                parse_quantity_full(&s).unwrap(),
                QuantityAmount::Overflow { positive: true },
                "overflow.add(1) should stay overflow"
            ),
            other => panic!("expected Quantity, got {:?}", other),
        }
    }

    #[test]
    fn test_sub_overflow_operand() {
        // overflow - finite → overflow (positive)
        let lhs = unsafe { make_quantity(OVERFLOW_POS) };
        let rhs = unsafe { make_quantity("1k") };
        let result = unsafe { read_val(cel_k8s_quantity_sub(lhs, rhs)) };
        match result {
            CelValue::Quantity(s) => assert_eq!(
                parse_quantity_full(&s).unwrap(),
                QuantityAmount::Overflow { positive: true },
                "overflow - 1k should stay overflow"
            ),
            other => panic!("expected Quantity, got {:?}", other),
        }

        // finite - overflow → negative overflow (subtracting +overflow flips sign)
        let lhs = unsafe { make_quantity("1k") };
        let rhs = unsafe { make_quantity(OVERFLOW_POS) };
        let result = unsafe { read_val(cel_k8s_quantity_sub(lhs, rhs)) };
        match result {
            CelValue::Quantity(s) => assert_eq!(
                parse_quantity_full(&s).unwrap(),
                QuantityAmount::Overflow { positive: false },
                "1k - overflow should be negative overflow"
            ),
            other => panic!("expected Quantity, got {:?}", other),
        }
    }

    #[test]
    fn test_sub_int_overflow_operand() {
        let q = unsafe { make_quantity(OVERFLOW_POS) };
        let n = make_int(1);
        let result = unsafe { read_val(cel_k8s_quantity_sub_int(q, n)) };
        match result {
            CelValue::Quantity(s) => assert_eq!(
                parse_quantity_full(&s).unwrap(),
                QuantityAmount::Overflow { positive: true },
                "overflow.sub(1) should stay overflow"
            ),
            other => panic!("expected Quantity, got {:?}", other),
        }
    }

    #[rstest]
    #[case(OVERFLOW_POS, OVERFLOW_POS, true)]
    #[case(OVERFLOW_NEG, OVERFLOW_NEG, true)]
    #[case(OVERFLOW_POS, OVERFLOW_NEG, false)]
    #[case(OVERFLOW_POS, "1k", false)]
    fn test_quantities_equal_overflow(#[case] a: &str, #[case] b: &str, #[case] expected: bool) {
        assert_eq!(quantities_equal(a, b), expected, "{}=={}", a, b);
    }
}
