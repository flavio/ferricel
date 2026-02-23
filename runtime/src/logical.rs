//! Boolean logic operations on CelValue::Bool pointers.

use crate::helpers::{cel_create_bool, extract_bool};
use crate::types::CelValue;

#[unsafe(no_mangle)]
pub extern "C" fn cel_bool_and(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_bool(a_ptr);
    let b = extract_bool(b_ptr);
    cel_create_bool(if a && b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_bool_or(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_bool(a_ptr);
    let b = extract_bool(b_ptr);
    cel_create_bool(if a || b { 1 } else { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_bool_not(a_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_bool(a_ptr);
    cel_create_bool(if !a { 1 } else { 0 })
}
