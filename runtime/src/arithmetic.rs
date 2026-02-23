//! Arithmetic operations with overflow checking and division-by-zero protection.
//! All operations panic on overflow or invalid operations per CEL spec.

use crate::helpers::{cel_create_int, extract_int};
use crate::types::CelValue;

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_add(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    let result = a.checked_add(b).expect("integer overflow in addition");
    cel_create_int(result)
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
