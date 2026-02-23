//! Comparison operations returning CelValue::Bool pointers.

use crate::helpers::{cel_create_bool, extract_int};
use crate::types::CelValue;

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_eq(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a == b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_ne(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a != b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_gt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a > b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_lt(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a < b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_gte(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a >= b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_lte(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_int(a_ptr);
    let b = extract_int(b_ptr);
    cel_create_bool(if a <= b { 1 } else { 0 })
}
