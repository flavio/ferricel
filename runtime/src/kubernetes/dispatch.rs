//! Runtime dispatch for CEL method names shared across multiple Kubernetes types.
//!
//! Some CEL method names (`isLessThan`, `isGreaterThan`, `compareTo`) are defined
//! on more than one Kubernetes CEL type (currently `Semver` and `Quantity`).
//! Because the compiler resolves function calls by name only — with no type
//! information available — it routes all of these calls to the functions below.
//! Each function inspects the receiver type at runtime and forwards the call to
//! the appropriate type-specific implementation.

use crate::{error::create_error_value, types::CelValue};

/// `<receiver>.isLessThan(<rhs>)` — dispatches to `Quantity` or `Semver`.
///
/// # Safety
/// Both pointers must be valid, non-null pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_poly_is_less_than(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    if lhs_ptr.is_null() {
        return create_error_value("no such overload");
    }
    let lhs_val = unsafe { &*lhs_ptr };
    match lhs_val {
        CelValue::Quantity(_) => unsafe {
            crate::kubernetes::quantity::cel_k8s_quantity_is_less_than(lhs_ptr, rhs_ptr)
        },
        CelValue::Semver(_) => unsafe {
            crate::kubernetes::semver::cel_k8s_semver_is_less_than(lhs_ptr, rhs_ptr)
        },
        _ => create_error_value("no such overload"),
    }
}

/// `<receiver>.isGreaterThan(<rhs>)` — dispatches to `Quantity` or `Semver`.
///
/// # Safety
/// Both pointers must be valid, non-null pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_poly_is_greater_than(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    if lhs_ptr.is_null() {
        return create_error_value("no such overload");
    }
    let lhs_val = unsafe { &*lhs_ptr };
    match lhs_val {
        CelValue::Quantity(_) => unsafe {
            crate::kubernetes::quantity::cel_k8s_quantity_is_greater_than(lhs_ptr, rhs_ptr)
        },
        CelValue::Semver(_) => unsafe {
            crate::kubernetes::semver::cel_k8s_semver_is_greater_than(lhs_ptr, rhs_ptr)
        },
        _ => create_error_value("no such overload"),
    }
}

/// `<receiver>.compareTo(<rhs>)` — dispatches to `Quantity` or `Semver`.
///
/// # Safety
/// Both pointers must be valid, non-null pointers to `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_poly_compare_to(
    lhs_ptr: *mut CelValue,
    rhs_ptr: *mut CelValue,
) -> *mut CelValue {
    if lhs_ptr.is_null() {
        return create_error_value("no such overload");
    }
    let lhs_val = unsafe { &*lhs_ptr };
    match lhs_val {
        CelValue::Quantity(_) => unsafe {
            crate::kubernetes::quantity::cel_k8s_quantity_compare_to(lhs_ptr, rhs_ptr)
        },
        CelValue::Semver(_) => unsafe {
            crate::kubernetes::semver::cel_k8s_semver_compare_to(lhs_ptr, rhs_ptr)
        },
        _ => create_error_value("no such overload"),
    }
}
