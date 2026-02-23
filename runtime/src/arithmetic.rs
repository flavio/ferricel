//! Arithmetic operations with overflow checking and division-by-zero protection.
//! All operations panic on overflow or invalid operations per CEL spec.

use crate::helpers::{cel_create_int, extract_int};
use crate::types::CelValue;

/// Internal helper: Add two integers with overflow checking.
///
/// # Arguments
/// - `a`: First integer operand
/// - `b`: Second integer operand
///
/// # Returns
/// The sum of `a` and `b`
///
/// # Panics
/// Panics on integer overflow
pub(crate) fn cel_int_add(a: i64, b: i64) -> i64 {
    a.checked_add(b).expect("integer overflow in addition")
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_sub(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    let result = a.checked_sub(b).expect("integer overflow in subtraction");
    cel_create_int(result)
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_mul(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    let result = a
        .checked_mul(b)
        .expect("integer overflow in multiplication");
    cel_create_int(result)
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_div(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    if b == 0 {
        panic!("division by zero");
    }
    // checked_div also catches the special case: i64::MIN / -1
    let result = a.checked_div(b).expect("integer overflow in division");
    cel_create_int(result)
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_mod(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    if b == 0 {
        panic!("modulo by zero");
    }
    // checked_rem also catches the special case: i64::MIN % -1
    let result = a.checked_rem(b).expect("integer overflow in modulo");
    cel_create_int(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int_add_basic() {
        let result = cel_int_add(2, 3);
        assert_eq!(result, 5);
    }

    #[test]
    fn test_int_add_negative() {
        let result = cel_int_add(-5, 3);
        assert_eq!(result, -2);
    }

    #[test]
    #[should_panic(expected = "integer overflow in addition")]
    fn test_int_add_overflow() {
        cel_int_add(i64::MAX, 1);
    }
}
