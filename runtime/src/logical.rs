//! Boolean logic operations on CelValue::Bool pointers.

use crate::error::create_error_value;
use crate::helpers::{cel_create_bool, extract_bool};
use crate::types::CelValue;

/// Boolean AND operator with short-circuit semantics.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_bool_and(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    unsafe {
        // CEL AND semantics (order matters!):
        // 1. Check short-circuit: false && X => false (don't care about X type)
        // 2. Check short-circuit: X && false => false (don't care about X type)
        // 3. Type check remaining operands
        // 4. Handle errors
        // 5. Apply boolean AND

        // Check if left is false (short-circuit, absorbs right)
        if !a_ptr.is_null()
            && let CelValue::Bool(false) = &*a_ptr
        {
            return a_ptr; // false && X => false (X not checked)
        }

        // Check if right is false (short-circuit, absorbs left errors)
        if !b_ptr.is_null()
            && let CelValue::Bool(false) = &*b_ptr
        {
            return b_ptr; // X && false => false (X error absorbed)
        }

        // Now type check: both operands must be Bool or Error
        if !a_ptr.is_null() {
            match &*a_ptr {
                CelValue::Bool(_) | CelValue::Error(_) => {}
                _ => return create_error_value("no such overload"),
            }
        }

        if !b_ptr.is_null() {
            match &*b_ptr {
                CelValue::Bool(_) | CelValue::Error(_) => {}
                _ => return create_error_value("no such overload"),
            }
        }

        // Handle errors (after short-circuit and type check)
        if !a_ptr.is_null()
            && let CelValue::Error(_) = &*a_ptr
        {
            return a_ptr; // error && true => error
        }

        if !b_ptr.is_null()
            && let CelValue::Error(_) = &*b_ptr
        {
            return b_ptr; // true && error => error
        }

        // Both are true, return true
        let a = extract_bool(a_ptr);
        let b = extract_bool(b_ptr);
        cel_create_bool(if a && b { 1 } else { 0 })
    }
}

/// Boolean OR operator with short-circuit semantics.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - Both pointer arguments are valid and properly aligned
/// - Both pointers point to initialized CelValue instances
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_bool_or(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    unsafe {
        // CEL OR semantics (order matters!):
        // 1. Check short-circuit: true || X => true (don't care about X type)
        // 2. Check short-circuit: X || true => true (don't care about X type)
        // 3. Type check remaining operands
        // 4. Handle errors
        // 5. Apply boolean OR

        // Check if left is true (short-circuit, absorbs right)
        if !a_ptr.is_null()
            && let CelValue::Bool(true) = &*a_ptr
        {
            return a_ptr; // true || X => true (X not checked)
        }

        // Check if right is true (short-circuit, absorbs left errors)
        if !b_ptr.is_null()
            && let CelValue::Bool(true) = &*b_ptr
        {
            return b_ptr; // X || true => true (X error absorbed)
        }

        // Now type check: both operands must be Bool or Error
        if !a_ptr.is_null() {
            match &*a_ptr {
                CelValue::Bool(_) | CelValue::Error(_) => {}
                _ => return create_error_value("no such overload"),
            }
        }

        if !b_ptr.is_null() {
            match &*b_ptr {
                CelValue::Bool(_) | CelValue::Error(_) => {}
                _ => return create_error_value("no such overload"),
            }
        }

        // Handle errors (after short-circuit and type check)
        if !a_ptr.is_null()
            && let CelValue::Error(_) = &*a_ptr
        {
            return a_ptr; // error || false => error
        }

        if !b_ptr.is_null()
            && let CelValue::Error(_) = &*b_ptr
        {
            return b_ptr; // false || error => error
        }

        // Both are false, return false
        let a = extract_bool(a_ptr);
        let b = extract_bool(b_ptr);
        cel_create_bool(if a || b { 1 } else { 0 })
    }
}

/// Boolean NOT operator.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned
/// - The pointer points to an initialized CelValue instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_bool_not(a_ptr: *mut CelValue) -> *mut CelValue {
    let a = extract_bool(a_ptr);
    cel_create_bool(if !a { 1 } else { 0 })
}

/// Check if a CelValue is NOT strictly false.
/// Used for comprehension short-circuiting in all() macro.
/// Returns true (1) if the value is anything other than CelValue::Bool(false).
/// This includes true, null, errors, and other types.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned (if not null)
/// - If not null, the pointer points to an initialized CelValue instance
/// - The returned pointer must be freed using the appropriate cleanup function
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_not_strictly_false(ptr: *mut CelValue) -> *mut CelValue {
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

/// Check if a CelValue is strictly false.
/// Returns 1 if value is CelValue::Bool(false), 0 otherwise.
/// Used for conditional short-circuit evaluation of && operator.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned (if not null)
/// - If not null, the pointer points to an initialized CelValue instance
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_is_strictly_false(ptr: *mut CelValue) -> i32 {
    unsafe {
        if ptr.is_null() {
            return 0;
        }
        match &*ptr {
            CelValue::Bool(false) => 1,
            _ => 0,
        }
    }
}

/// Check if a CelValue is strictly true.
/// Returns 1 if value is CelValue::Bool(true), 0 otherwise.
/// Used for conditional short-circuit evaluation of || operator.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned (if not null)
/// - If not null, the pointer points to an initialized CelValue instance
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_is_strictly_true(ptr: *mut CelValue) -> i32 {
    unsafe {
        if ptr.is_null() {
            return 0;
        }
        match &*ptr {
            CelValue::Bool(true) => 1,
            _ => 0,
        }
    }
}

/// Check if a CelValue is an error.
/// Returns 1 if value is CelValue::Error, 0 otherwise.
/// Used for error propagation and absorption in logical operators.
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers. The caller must ensure:
/// - The pointer argument is valid and properly aligned (if not null)
/// - If not null, the pointer points to an initialized CelValue instance
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_is_error(ptr: *mut CelValue) -> i32 {
    unsafe {
        if ptr.is_null() {
            return 0;
        }
        match &*ptr {
            CelValue::Error(_) => 1,
            _ => 0,
        }
    }
}
