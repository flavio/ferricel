//! Boolean logic operations on CelValue::Bool pointers.

use crate::types::CelValue;

// ---------------------------------------------------------------------------
// Consuming operator functions (ABI boundary — take ownership of inputs)
// ---------------------------------------------------------------------------

/// Boolean AND operator. Consumes both operands.
///
/// CEL AND semantics:
/// - false && X => false  (type of X not checked)
/// - X && false => false  (errors in X absorbed)
/// - error && true => error
/// - true && error => error
/// - true && true => true
/// - non-bool operand => error "no such overload"
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_bool_and(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = unsafe { *Box::from_raw(a_ptr) };
    let b = unsafe { *Box::from_raw(b_ptr) };
    Box::into_raw(Box::new(bool_and(a, b)))
}

fn bool_and(a: CelValue, b: CelValue) -> CelValue {
    match (a, b) {
        // Short-circuit: either side is false → false (absorbs errors on the other side)
        (CelValue::Bool(false), _) => CelValue::Bool(false),
        (_, CelValue::Bool(false)) => CelValue::Bool(false),
        // Type errors: non-bool, non-error operand
        (other, _) if !matches!(other, CelValue::Bool(_) | CelValue::Error(_)) => {
            CelValue::Error("no such overload".into())
        }
        (_, other) if !matches!(other, CelValue::Bool(_) | CelValue::Error(_)) => {
            CelValue::Error("no such overload".into())
        }
        // Error propagation (after short-circuit)
        (CelValue::Error(e), _) => CelValue::Error(e),
        (_, CelValue::Error(e)) => CelValue::Error(e),
        // Both true
        (CelValue::Bool(a), CelValue::Bool(b)) => CelValue::Bool(a && b),
        // Unreachable
        _ => CelValue::Error("no such overload".into()),
    }
}

/// Boolean OR operator. Consumes both operands.
///
/// CEL OR semantics:
/// - true || X => true  (type of X not checked)
/// - X || true => true  (errors in X absorbed)
/// - error || false => error
/// - false || error => error
/// - false || false => false
/// - non-bool operand => error "no such overload"
///
/// # Safety
/// Both pointers must be valid, non-null, uniquely-owned CelValue pointers.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_bool_or(a_ptr: *mut CelValue, b_ptr: *mut CelValue) -> *mut CelValue {
    let a = unsafe { *Box::from_raw(a_ptr) };
    let b = unsafe { *Box::from_raw(b_ptr) };
    Box::into_raw(Box::new(bool_or(a, b)))
}

fn bool_or(a: CelValue, b: CelValue) -> CelValue {
    match (a, b) {
        // Short-circuit: either side is true → true (absorbs errors on the other side)
        (CelValue::Bool(true), _) => CelValue::Bool(true),
        (_, CelValue::Bool(true)) => CelValue::Bool(true),
        // Type errors: non-bool, non-error operand
        (other, _) if !matches!(other, CelValue::Bool(_) | CelValue::Error(_)) => {
            CelValue::Error("no such overload".into())
        }
        (_, other) if !matches!(other, CelValue::Bool(_) | CelValue::Error(_)) => {
            CelValue::Error("no such overload".into())
        }
        // Error propagation (after short-circuit)
        (CelValue::Error(e), _) => CelValue::Error(e),
        (_, CelValue::Error(e)) => CelValue::Error(e),
        // Both false
        (CelValue::Bool(a), CelValue::Bool(b)) => CelValue::Bool(a || b),
        // Unreachable
        _ => CelValue::Error("no such overload".into()),
    }
}

/// Boolean NOT operator. Consumes the operand.
///
/// # Safety
/// Pointer must be a valid, non-null, uniquely-owned CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_bool_not(a_ptr: *mut CelValue) -> *mut CelValue {
    let a = unsafe { *Box::from_raw(a_ptr) };
    let result = match a {
        CelValue::Bool(b) => CelValue::Bool(!b),
        CelValue::Error(e) => CelValue::Error(e),
        _ => CelValue::Error("no such overload".into()),
    };
    Box::into_raw(Box::new(result))
}

/// @not_strictly_false operator. Consumes the operand.
/// Returns true for anything that is not CelValue::Bool(false).
///
/// # Safety
/// Pointer must be a valid, non-null, uniquely-owned CelValue pointer.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_not_strictly_false(ptr: *mut CelValue) -> *mut CelValue {
    let value = unsafe { *Box::from_raw(ptr) };
    let result = !matches!(value, CelValue::Bool(false));
    Box::into_raw(Box::new(CelValue::Bool(result)))
}

// ---------------------------------------------------------------------------
// Non-consuming query functions (called by compiler short-circuit control flow)
// These BORROW the pointer — they do NOT take ownership.
// ---------------------------------------------------------------------------

/// Check if a CelValue is strictly false. Returns 1 if Bool(false), 0 otherwise.
/// Used by the compiler for && short-circuit control flow.
///
/// # Safety
/// Pointer must be a valid (possibly null) CelValue pointer. Does NOT consume.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_is_strictly_false(ptr: *mut CelValue) -> i32 {
    if ptr.is_null() {
        return 0;
    }
    match unsafe { &*ptr } {
        CelValue::Bool(false) => 1,
        _ => 0,
    }
}

/// Check if a CelValue is strictly true. Returns 1 if Bool(true), 0 otherwise.
/// Used by the compiler for || short-circuit control flow.
///
/// # Safety
/// Pointer must be a valid (possibly null) CelValue pointer. Does NOT consume.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_is_strictly_true(ptr: *mut CelValue) -> i32 {
    if ptr.is_null() {
        return 0;
    }
    match unsafe { &*ptr } {
        CelValue::Bool(true) => 1,
        _ => 0,
    }
}

/// Check if a CelValue is an error. Returns 1 if Error, 0 otherwise.
/// Used by the compiler for conditional/ternary error propagation.
///
/// # Safety
/// Pointer must be a valid (possibly null) CelValue pointer. Does NOT consume.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_is_error(ptr: *mut CelValue) -> i32 {
    if ptr.is_null() {
        return 0;
    }
    match unsafe { &*ptr } {
        CelValue::Error(_) => 1,
        _ => 0,
    }
}
