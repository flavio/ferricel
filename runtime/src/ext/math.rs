//! CEL math extension library functions.
//!
//! Implements `math.greatest`, `math.least`, `math.ceil`, `math.floor`,
//! `math.round`, `math.trunc`, `math.abs`, `math.sign`, `math.isInf`,
//! `math.isNaN`, `math.isFinite`, `math.bitAnd`, `math.bitOr`, `math.bitXor`,
//! `math.bitNot`, `math.bitShiftLeft`, `math.bitShiftRight`, `math.sqrt`.
//!
//! See: <https://pkg.go.dev/github.com/google/cel-go/ext#Math>

use crate::error::read_ptr;
use crate::types::CelValue;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Convert a numeric `CelValue` to `f64` for cross-type comparisons.
/// Returns `None` for non-numeric types.
fn to_f64(v: &CelValue) -> Option<f64> {
    match v {
        CelValue::Int(i) => Some(*i as f64),
        CelValue::UInt(u) => Some(*u as f64),
        CelValue::Double(d) => Some(*d),
        _ => None,
    }
}

/// Compare two numeric `CelValue`s. Returns `Some(Ordering)` if both are numeric,
/// `None` if either is not numeric.
fn numeric_cmp(a: &CelValue, b: &CelValue) -> Option<std::cmp::Ordering> {
    let fa = to_f64(a)?;
    let fb = to_f64(b)?;
    fa.partial_cmp(&fb)
}

/// Error value for "no such overload".
fn no_such_overload(msg: &str) -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::Error(format!(
        "no such overload: {}",
        msg
    ))))
}

/// Error value for a general runtime error.
fn runtime_error(msg: impl Into<String>) -> *mut CelValue {
    Box::into_raw(Box::new(CelValue::Error(msg.into())))
}

/// Find the greatest value in a list. Returns `Ok(*mut CelValue)` or `Err(error_ptr)`.
fn greatest_in_list(arr: &[CelValue]) -> *mut CelValue {
    if arr.is_empty() {
        return runtime_error("math.@max(list) argument must not be empty");
    }
    let mut best = &arr[0];
    if to_f64(best).is_none() {
        return no_such_overload("math.@max");
    }
    for item in arr.iter().skip(1) {
        match numeric_cmp(item, best) {
            Some(std::cmp::Ordering::Greater) => best = item,
            None => return no_such_overload("math.@max"),
            _ => {}
        }
    }
    Box::into_raw(Box::new(best.clone()))
}

/// Find the least value in a list. Returns a `*mut CelValue` (value or error).
fn least_in_list(arr: &[CelValue]) -> *mut CelValue {
    if arr.is_empty() {
        return runtime_error("math.@min(list) argument must not be empty");
    }
    let mut best = &arr[0];
    if to_f64(best).is_none() {
        return no_such_overload("math.@min");
    }
    for item in arr.iter().skip(1) {
        match numeric_cmp(item, best) {
            Some(std::cmp::Ordering::Less) => best = item,
            None => return no_such_overload("math.@min"),
            _ => {}
        }
    }
    Box::into_raw(Box::new(best.clone()))
}

// ---------------------------------------------------------------------------
// math.greatest / math.least
// ---------------------------------------------------------------------------

/// `math.greatest(list_or_scalar) -> numeric`
///
/// Accepts either a single scalar numeric or a list of numerics and returns
/// the largest value. The compiler wraps multi-argument calls into a list
/// before invoking this function.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_greatest(val_ptr: *mut CelValue) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    match val {
        CelValue::Array(ref arr) => greatest_in_list(arr),
        CelValue::Int(_) | CelValue::UInt(_) | CelValue::Double(_) => Box::into_raw(Box::new(val)),
        _ => no_such_overload("math.@max"),
    }
}

/// `math.least(list_or_scalar) -> numeric`
///
/// Accepts either a single scalar numeric or a list of numerics and returns
/// the smallest value. The compiler wraps multi-argument calls into a list
/// before invoking this function.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_least(val_ptr: *mut CelValue) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    match val {
        CelValue::Array(ref arr) => least_in_list(arr),
        CelValue::Int(_) | CelValue::UInt(_) | CelValue::Double(_) => Box::into_raw(Box::new(val)),
        _ => no_such_overload("math.@min"),
    }
}

// ---------------------------------------------------------------------------
// Rounding: ceil, floor, round, trunc  (double -> double)
// ---------------------------------------------------------------------------

/// `math.ceil(double) -> double`
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_ceil(val_ptr: *mut CelValue) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    match val {
        CelValue::Double(d) => Box::into_raw(Box::new(CelValue::Double(d.ceil()))),
        _ => no_such_overload("math.ceil(double)"),
    }
}

/// `math.floor(double) -> double`
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_floor(val_ptr: *mut CelValue) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    match val {
        CelValue::Double(d) => Box::into_raw(Box::new(CelValue::Double(d.floor()))),
        _ => no_such_overload("math.floor(double)"),
    }
}

/// `math.round(double) -> double`
///
/// Rounds to nearest, ties away from zero (1.5 -> 2.0, -1.5 -> -2.0).
/// Rust's `f64::round()` already uses this convention.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_round(val_ptr: *mut CelValue) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    match val {
        CelValue::Double(d) => Box::into_raw(Box::new(CelValue::Double(d.round()))),
        _ => no_such_overload("math.round(double)"),
    }
}

/// `math.trunc(double) -> double`
///
/// Truncates the fractional portion (rounds toward zero).
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_trunc(val_ptr: *mut CelValue) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    match val {
        CelValue::Double(d) => Box::into_raw(Box::new(CelValue::Double(d.trunc()))),
        _ => no_such_overload("math.trunc(double)"),
    }
}

// ---------------------------------------------------------------------------
// abs / sign  (int, uint, double overloads)
// ---------------------------------------------------------------------------

/// `math.abs(int|uint|double) -> int|uint|double`
///
/// Returns the absolute value. If the input is `int64::MIN` an overflow error
/// is returned (there is no positive counterpart in two's complement).
/// For NaN inputs the output is NaN.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_abs(val_ptr: *mut CelValue) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    match val {
        CelValue::Int(i) => match i.checked_abs() {
            Some(v) => Box::into_raw(Box::new(CelValue::Int(v))),
            None => runtime_error("overflow"),
        },
        CelValue::UInt(u) => Box::into_raw(Box::new(CelValue::UInt(u))),
        CelValue::Double(d) => Box::into_raw(Box::new(CelValue::Double(d.abs()))),
        _ => no_such_overload("math.abs"),
    }
}

/// `math.sign(int|uint|double) -> int|uint|double`
///
/// Returns -1, 0, or 1 as an int/uint/double depending on the type of the input.
/// For double NaN the output is NaN. Does not differentiate +0.0 and -0.0.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_sign(val_ptr: *mut CelValue) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    match val {
        CelValue::Int(i) => Box::into_raw(Box::new(CelValue::Int(i.signum()))),
        CelValue::UInt(u) => {
            let sign: u64 = if u == 0 { 0 } else { 1 };
            Box::into_raw(Box::new(CelValue::UInt(sign)))
        }
        CelValue::Double(d) => {
            let sign = if d.is_nan() {
                f64::NAN
            } else if d == 0.0 {
                // CEL: don't differentiate +0.0 and -0.0
                0.0_f64
            } else {
                d.signum()
            };
            Box::into_raw(Box::new(CelValue::Double(sign)))
        }
        _ => no_such_overload("math.sign"),
    }
}

// ---------------------------------------------------------------------------
// Float predicates: isInf, isNaN, isFinite  (double -> bool)
// ---------------------------------------------------------------------------

/// `math.isInf(double) -> bool`
///
/// Returns true if the value is positive or negative infinity.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_is_inf(val_ptr: *mut CelValue) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    match val {
        CelValue::Double(d) => Box::into_raw(Box::new(CelValue::Bool(d.is_infinite()))),
        _ => no_such_overload("math.isInf(double)"),
    }
}

/// `math.isNaN(double) -> bool`
///
/// Returns true if the value is NaN.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_is_nan(val_ptr: *mut CelValue) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    match val {
        CelValue::Double(d) => Box::into_raw(Box::new(CelValue::Bool(d.is_nan()))),
        _ => no_such_overload("math.isNaN(double)"),
    }
}

/// `math.isFinite(double) -> bool`
///
/// Returns true if the value is neither NaN nor infinite.
/// Equivalent to `!math.isNaN(d) && !math.isInf(d)`.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_is_finite(val_ptr: *mut CelValue) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    match val {
        CelValue::Double(d) => Box::into_raw(Box::new(CelValue::Bool(d.is_finite()))),
        _ => no_such_overload("math.isFinite(double)"),
    }
}

// ---------------------------------------------------------------------------
// Bitwise operations (int/uint overloads)
// ---------------------------------------------------------------------------

/// `math.bitOr(int, int) -> int` / `math.bitOr(uint, uint) -> uint`
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_bit_or(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    let lhs = unsafe { read_ptr(lhs_ptr) };
    let rhs = unsafe { read_ptr(rhs_ptr) };
    let type_names = (lhs.type_name(), rhs.type_name());
    match (lhs, rhs) {
        (CelValue::Int(a), CelValue::Int(b)) => Box::into_raw(Box::new(CelValue::Int(a | b))),
        (CelValue::UInt(a), CelValue::UInt(b)) => Box::into_raw(Box::new(CelValue::UInt(a | b))),
        _ => no_such_overload(&format!("math.bitOr({}, {})", type_names.0, type_names.1)),
    }
}

/// `math.bitAnd(int, int) -> int` / `math.bitAnd(uint, uint) -> uint`
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_bit_and(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    let lhs = unsafe { read_ptr(lhs_ptr) };
    let rhs = unsafe { read_ptr(rhs_ptr) };
    let type_names = (lhs.type_name(), rhs.type_name());
    match (lhs, rhs) {
        (CelValue::Int(a), CelValue::Int(b)) => Box::into_raw(Box::new(CelValue::Int(a & b))),
        (CelValue::UInt(a), CelValue::UInt(b)) => Box::into_raw(Box::new(CelValue::UInt(a & b))),
        _ => no_such_overload(&format!("math.bitAnd({}, {})", type_names.0, type_names.1)),
    }
}

/// `math.bitXor(int, int) -> int` / `math.bitXor(uint, uint) -> uint`
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_bit_xor(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    let lhs = unsafe { read_ptr(lhs_ptr) };
    let rhs = unsafe { read_ptr(rhs_ptr) };
    let type_names = (lhs.type_name(), rhs.type_name());
    match (lhs, rhs) {
        (CelValue::Int(a), CelValue::Int(b)) => Box::into_raw(Box::new(CelValue::Int(a ^ b))),
        (CelValue::UInt(a), CelValue::UInt(b)) => Box::into_raw(Box::new(CelValue::UInt(a ^ b))),
        _ => no_such_overload(&format!("math.bitXor({}, {})", type_names.0, type_names.1)),
    }
}

/// `math.bitNot(int) -> int` / `math.bitNot(uint) -> uint`
///
/// Performs a bitwise-NOT (one's complement).
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_bit_not(val_ptr: *mut CelValue) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    let type_name = val.type_name();
    match val {
        CelValue::Int(i) => Box::into_raw(Box::new(CelValue::Int(!i))),
        CelValue::UInt(u) => Box::into_raw(Box::new(CelValue::UInt(!u))),
        _ => no_such_overload(&format!("math.bitNot({})", type_name)),
    }
}

/// `math.bitShiftLeft(int|uint, int) -> int|uint`
///
/// Shifts left by `offset` bits. If `offset >= 64` the result is 0.
/// A negative offset is a runtime error.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_bit_shift_left(
    val_ptr: *mut CelValue,
    offset_ptr: *mut CelValue,
) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    let offset_val = unsafe { read_ptr(offset_ptr) };
    let offset = match offset_val {
        CelValue::Int(i) => i,
        ref v => return no_such_overload(&format!("math.bitShiftLeft(_, {})", v.type_name())),
    };
    if offset < 0 {
        return runtime_error("math.bitShiftLeft() negative offset");
    }
    let shift = offset as u32;
    let val_type_name = val.type_name();
    match val {
        CelValue::Int(i) => {
            let result = if shift >= 64 {
                0i64
            } else {
                i.wrapping_shl(shift)
            };
            Box::into_raw(Box::new(CelValue::Int(result)))
        }
        CelValue::UInt(u) => {
            let result = if shift >= 64 {
                0u64
            } else {
                u.wrapping_shl(shift)
            };
            Box::into_raw(Box::new(CelValue::UInt(result)))
        }
        _ => no_such_overload(&format!("math.bitShiftLeft({}, int)", val_type_name)),
    }
}

/// `math.bitShiftRight(int|uint, int) -> int|uint`
///
/// Performs a **logical** (unsigned) right shift — sign bit is NOT preserved.
/// If `offset >= 64` the result is 0. A negative offset is a runtime error.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_bit_shift_right(
    val_ptr: *mut CelValue,
    offset_ptr: *mut CelValue,
) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    let offset_val = unsafe { read_ptr(offset_ptr) };
    let offset = match offset_val {
        CelValue::Int(i) => i,
        ref v => return no_such_overload(&format!("math.bitShiftRight(_, {})", v.type_name())),
    };
    if offset < 0 {
        return runtime_error("math.bitShiftRight() negative offset");
    }
    let shift = offset as u32;
    let val_type_name = val.type_name();
    match val {
        CelValue::Int(i) => {
            // Logical (unsigned) right shift: cast to u64, shift, cast back.
            let result = if shift >= 64 {
                0i64
            } else {
                ((i as u64) >> shift) as i64
            };
            Box::into_raw(Box::new(CelValue::Int(result)))
        }
        CelValue::UInt(u) => {
            let result = if shift >= 64 { 0u64 } else { u >> shift };
            Box::into_raw(Box::new(CelValue::UInt(result)))
        }
        _ => no_such_overload(&format!("math.bitShiftRight({}, int)", val_type_name)),
    }
}

// ---------------------------------------------------------------------------
// math.sqrt  (int|uint|double -> double)
// ---------------------------------------------------------------------------

/// `math.sqrt(int|uint|double) -> double`
///
/// Returns the square root as a double. For negative inputs returns NaN.
///
/// # Safety
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_math_sqrt(val_ptr: *mut CelValue) -> *mut CelValue {
    let val = unsafe { read_ptr(val_ptr) };
    let type_name = val.type_name();
    let d = match val {
        CelValue::Int(i) => i as f64,
        CelValue::UInt(u) => u as f64,
        CelValue::Double(d) => d,
        _ => return no_such_overload(&format!("math.sqrt({})", type_name)),
    };
    Box::into_raw(Box::new(CelValue::Double(d.sqrt())))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // ── helpers ───────────────────────────────────────────────────────────────

    /// Call a unary FFI function with an owned `CelValue` and return the owned result.
    unsafe fn call1(
        f: unsafe extern "C" fn(*mut CelValue) -> *mut CelValue,
        v: CelValue,
    ) -> CelValue {
        let result_ptr = unsafe { f(Box::into_raw(Box::new(v))) };
        let result = unsafe { (*result_ptr).clone() };
        result
    }

    /// Call a binary FFI function with two owned `CelValue`s.
    unsafe fn call2(
        f: unsafe extern "C" fn(*mut CelValue, *mut CelValue) -> *mut CelValue,
        a: CelValue,
        b: CelValue,
    ) -> CelValue {
        let result_ptr = unsafe { f(Box::into_raw(Box::new(a)), Box::into_raw(Box::new(b))) };
        let result = unsafe { (*result_ptr).clone() };
        result
    }

    fn is_error(v: &CelValue) -> bool {
        matches!(v, CelValue::Error(_))
    }

    fn error_msg(v: &CelValue) -> &str {
        match v {
            CelValue::Error(msg) => msg.as_str(),
            _ => panic!("not an error: {:?}", v),
        }
    }

    // ── greatest / least: empty list ─────────────────────────────────────────

    #[test]
    fn test_greatest_empty_list_is_error() {
        let result = unsafe { call1(cel_math_greatest, CelValue::Array(vec![])) };
        assert!(is_error(&result));
    }

    #[test]
    fn test_least_empty_list_is_error() {
        let result = unsafe { call1(cel_math_least, CelValue::Array(vec![])) };
        assert!(is_error(&result));
    }

    // ── greatest / least: non-numeric element ────────────────────────────────

    #[test]
    fn test_greatest_non_numeric_first_element_is_overload_error() {
        let arr = CelValue::Array(vec![CelValue::String("x".to_string()), CelValue::Int(1)]);
        let result = unsafe { call1(cel_math_greatest, arr) };
        assert!(error_msg(&result).contains("no such overload"));
    }

    #[test]
    fn test_greatest_non_numeric_mid_element_is_overload_error() {
        let arr = CelValue::Array(vec![
            CelValue::Int(1),
            CelValue::String("x".to_string()),
            CelValue::Int(3),
        ]);
        let result = unsafe { call1(cel_math_greatest, arr) };
        assert!(error_msg(&result).contains("no such overload"));
    }

    #[test]
    fn test_least_non_numeric_mid_element_is_overload_error() {
        let arr = CelValue::Array(vec![CelValue::Int(1), CelValue::String("x".to_string())]);
        let result = unsafe { call1(cel_math_least, arr) };
        assert!(error_msg(&result).contains("no such overload"));
    }

    // ── greatest / least: mixed numeric types preserve winner's type ─────────

    #[test]
    fn test_greatest_mixed_types_preserves_winner_type() {
        // Int(1) vs Double(2.0) — winner is Double(2.0)
        let arr = CelValue::Array(vec![CelValue::Int(1), CelValue::Double(2.0)]);
        let result = unsafe { call1(cel_math_greatest, arr) };
        assert_eq!(result, CelValue::Double(2.0));
    }

    #[test]
    fn test_least_mixed_types_preserves_winner_type() {
        // Int(1) vs Double(2.0) — winner is Int(1)
        let arr = CelValue::Array(vec![CelValue::Int(1), CelValue::Double(2.0)]);
        let result = unsafe { call1(cel_math_least, arr) };
        assert_eq!(result, CelValue::Int(1));
    }

    #[test]
    fn test_greatest_uint_vs_int_preserves_winner_type() {
        // UInt(5) vs Int(3) — winner is UInt(5)
        let arr = CelValue::Array(vec![CelValue::UInt(5), CelValue::Int(3)]);
        let result = unsafe { call1(cel_math_greatest, arr) };
        assert_eq!(result, CelValue::UInt(5));
    }

    // ── greatest / least: scalar passthrough preserves type ──────────────────

    #[rstest]
    #[case::int(CelValue::Int(42))]
    #[case::uint(CelValue::UInt(42))]
    #[case::double(CelValue::Double(42.0))]
    fn test_greatest_scalar_passthrough(#[case] input: CelValue) {
        let result = unsafe { call1(cel_math_greatest, input.clone()) };
        assert_eq!(result, input);
    }

    #[rstest]
    #[case::int(CelValue::Int(42))]
    #[case::uint(CelValue::UInt(42))]
    #[case::double(CelValue::Double(42.0))]
    fn test_least_scalar_passthrough(#[case] input: CelValue) {
        let result = unsafe { call1(cel_math_least, input.clone()) };
        assert_eq!(result, input);
    }

    // ── greatest / least: non-numeric scalar ─────────────────────────────────

    #[test]
    fn test_greatest_non_numeric_scalar_is_overload_error() {
        let result = unsafe { call1(cel_math_greatest, CelValue::String("x".to_string())) };
        assert!(error_msg(&result).contains("no such overload"));
    }

    #[test]
    fn test_least_non_numeric_scalar_is_overload_error() {
        let result = unsafe { call1(cel_math_least, CelValue::Bool(true)) };
        assert!(error_msg(&result).contains("no such overload"));
    }

    // ── abs: i64::MIN overflow ────────────────────────────────────────────────

    #[test]
    fn test_abs_int_min_overflow() {
        let result = unsafe { call1(cel_math_abs, CelValue::Int(i64::MIN)) };
        assert!(error_msg(&result).contains("overflow"));
    }

    // ── sign: +0.0 and -0.0 both return 0.0 ─────────────────────────────────

    #[test]
    fn test_sign_positive_zero_returns_zero() {
        let result = unsafe { call1(cel_math_sign, CelValue::Double(0.0_f64)) };
        assert_eq!(result, CelValue::Double(0.0));
    }

    #[test]
    fn test_sign_negative_zero_returns_zero() {
        let result = unsafe { call1(cel_math_sign, CelValue::Double(-0.0_f64)) };
        assert_eq!(result, CelValue::Double(0.0));
        // Confirm sign bit is not set (Rust's signum would give -1.0 for -0.0)
        if let CelValue::Double(d) = result {
            assert!(!d.is_sign_negative());
        }
    }

    #[test]
    fn test_sign_nan_returns_nan() {
        let result = unsafe { call1(cel_math_sign, CelValue::Double(f64::NAN)) };
        assert!(matches!(result, CelValue::Double(d) if d.is_nan()));
    }

    // ── bitShiftLeft: negative offset is a runtime error ─────────────────────

    #[test]
    fn test_bit_shift_left_negative_offset_is_error() {
        let result = unsafe { call2(cel_math_bit_shift_left, CelValue::Int(1), CelValue::Int(-1)) };
        assert!(is_error(&result));
        assert!(!error_msg(&result).contains("no such overload"));
    }

    // ── bitShiftLeft: shift >= 64 saturates to 0 ─────────────────────────────

    #[rstest]
    #[case::exactly_64(64)]
    #[case::large(200)]
    fn test_bit_shift_left_large_shift_saturates_int(#[case] shift: i64) {
        let result = unsafe {
            call2(
                cel_math_bit_shift_left,
                CelValue::Int(1),
                CelValue::Int(shift),
            )
        };
        assert_eq!(result, CelValue::Int(0));
    }

    #[rstest]
    #[case::exactly_64(64)]
    #[case::large(200)]
    fn test_bit_shift_left_large_shift_saturates_uint(#[case] shift: i64) {
        let result = unsafe {
            call2(
                cel_math_bit_shift_left,
                CelValue::UInt(1),
                CelValue::Int(shift),
            )
        };
        assert_eq!(result, CelValue::UInt(0));
    }

    // ── bitShiftRight: negative offset is a runtime error ────────────────────

    #[test]
    fn test_bit_shift_right_negative_offset_is_error() {
        let result = unsafe {
            call2(
                cel_math_bit_shift_right,
                CelValue::Int(1),
                CelValue::Int(-1),
            )
        };
        assert!(is_error(&result));
        assert!(!error_msg(&result).contains("no such overload"));
    }

    // ── bitShiftRight: shift >= 64 saturates to 0 ────────────────────────────

    #[rstest]
    #[case::exactly_64(64)]
    #[case::large(200)]
    fn test_bit_shift_right_large_shift_saturates_int(#[case] shift: i64) {
        let result = unsafe {
            call2(
                cel_math_bit_shift_right,
                CelValue::Int(-1),
                CelValue::Int(shift),
            )
        };
        assert_eq!(result, CelValue::Int(0));
    }

    #[rstest]
    #[case::exactly_64(64)]
    #[case::large(200)]
    fn test_bit_shift_right_large_shift_saturates_uint(#[case] shift: i64) {
        let result = unsafe {
            call2(
                cel_math_bit_shift_right,
                CelValue::UInt(u64::MAX),
                CelValue::Int(shift),
            )
        };
        assert_eq!(result, CelValue::UInt(0));
    }

    // ── bitShiftRight: logical (not arithmetic) shift on negative int ─────────

    #[test]
    fn test_bit_shift_right_negative_int_is_logical_not_arithmetic() {
        // -1024 >> 3: arithmetic would give -128, logical gives 2305843009213693824
        let result = unsafe {
            call2(
                cel_math_bit_shift_right,
                CelValue::Int(-1024),
                CelValue::Int(3),
            )
        };
        assert_eq!(result, CelValue::Int(2305843009213693824));
    }

    #[test]
    fn test_bit_shift_right_minus_one_logical() {
        // -1 >> 1: arithmetic would give -1, logical gives i64::MAX
        let result = unsafe {
            call2(
                cel_math_bit_shift_right,
                CelValue::Int(-1),
                CelValue::Int(1),
            )
        };
        assert_eq!(result, CelValue::Int(i64::MAX));
    }

    // ── type mismatch error messages include type names ───────────────────────

    #[test]
    fn test_bit_or_type_mismatch_error_contains_type_names() {
        let result = unsafe { call2(cel_math_bit_or, CelValue::Double(1.0), CelValue::Int(1)) };
        let msg = error_msg(&result);
        assert!(msg.contains("double"), "expected 'double' in: {msg}");
        assert!(msg.contains("int"), "expected 'int' in: {msg}");
    }

    #[test]
    fn test_bit_shift_left_wrong_offset_type_error_contains_type_name() {
        let result = unsafe {
            call2(
                cel_math_bit_shift_left,
                CelValue::Int(1),
                CelValue::Double(3.0),
            )
        };
        let msg = error_msg(&result);
        assert!(msg.contains("double"), "expected 'double' in: {msg}");
    }

    #[test]
    fn test_bit_shift_right_wrong_value_type_error_contains_type_name() {
        let result = unsafe {
            call2(
                cel_math_bit_shift_right,
                CelValue::String("x".to_string()),
                CelValue::Int(1),
            )
        };
        let msg = error_msg(&result);
        assert!(msg.contains("string"), "expected 'string' in: {msg}");
    }
}
