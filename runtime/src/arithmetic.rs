//! Arithmetic operations with overflow checking and division-by-zero protection.
//! All operations panic on overflow or invalid operations per CEL spec.

extern crate alloc;

use crate::helpers::{cel_create_int, extract_int};
use crate::types::CelValue;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Add two CelValue objects.
/// Supports:
/// - Int + Int = Int
/// - Array + Array = Array (concatenation)
#[unsafe(no_mangle)]
pub extern "C" fn cel_int_add(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    unsafe {
        if a_ptr.is_null() || b_ptr.is_null() {
            panic!("Cannot add null values");
        }

        let a_val = &*a_ptr;
        let b_val = &*b_ptr;

        match (a_val, b_val) {
            (CelValue::Int(a), CelValue::Int(b)) => {
                let result = a.checked_add(*b).expect("integer overflow in addition");
                cel_create_int(result)
            }
            (CelValue::Array(a_vec), CelValue::Array(b_vec)) => {
                // Concatenate two arrays
                let mut result_vec = Vec::with_capacity(a_vec.len() + b_vec.len());
                result_vec.extend_from_slice(a_vec);
                result_vec.extend_from_slice(b_vec);
                let result = CelValue::Array(result_vec);
                Box::into_raw(Box::new(result))
            }
            _ => panic!(
                "Cannot add {:?} and {:?}: unsupported types for addition",
                a_val, b_val
            ),
        }
    }
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
