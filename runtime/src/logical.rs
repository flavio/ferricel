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

/// Check if a CelValue is NOT strictly false.
/// Used for comprehension short-circuiting in all() macro.
/// Returns true (1) if the value is anything other than CelValue::Bool(false).
/// This includes true, null, errors, and other types.
#[unsafe(no_mangle)]
pub extern "C" fn cel_not_strictly_false(ptr: *mut CelValue) -> *mut CelValue {
    unsafe {
        if ptr.is_null() {
            // Null is not strictly false
            return cel_create_bool(1);
        }

        match &*ptr {
            CelValue::Bool(false) => cel_create_bool(0), // Strictly false
            _ => cel_create_bool(1),                     // Anything else is not strictly false
        }
    }
}

/// Ternary/conditional operator: condition ? true_value : false_value
/// If condition is true, returns true_value, otherwise returns false_value.
/// The condition is evaluated as a boolean using extract_bool.
#[unsafe(no_mangle)]
pub extern "C" fn cel_conditional(
    cond_ptr: *mut CelValue,
    true_ptr: *mut CelValue,
    false_ptr: *mut CelValue,
) -> *mut CelValue {
    let cond = extract_bool(cond_ptr);
    if cond {
        true_ptr
    } else {
        false_ptr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conditional_true() {
        let cond = cel_create_bool(1);
        let true_val = cel_create_bool(1);
        let false_val = cel_create_bool(0);
        let result = cel_conditional(cond, true_val, false_val);
        assert_eq!(
            result, true_val,
            "Should return true value when condition is true"
        );
    }

    #[test]
    fn test_conditional_false() {
        let cond = cel_create_bool(0);
        let true_val = cel_create_bool(1);
        let false_val = cel_create_bool(0);
        let result = cel_conditional(cond, true_val, false_val);
        assert_eq!(
            result, false_val,
            "Should return false value when condition is false"
        );
    }
}
