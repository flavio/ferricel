//! Kubernetes CEL extensions.
//!
//! This module contains implementations of the additional CEL functions that
//! Kubernetes adds on top of the standard CEL specification.
//! See: <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-cel-libraries>

pub mod lists;
pub mod regex;
pub mod url;

#[cfg(test)]
pub(super) mod test_helpers {
    use crate::types::CelValue;

    pub(super) unsafe fn read_val(ptr: *mut CelValue) -> CelValue {
        (*ptr).clone()
    }

    pub(super) unsafe fn make_val(v: CelValue) -> *mut CelValue {
        Box::into_raw(Box::new(v))
    }

    pub(super) unsafe fn make_str(s: &str) -> *mut CelValue {
        make_val(CelValue::String(s.to_string()))
    }

    pub(super) unsafe fn make_int(n: i64) -> *mut CelValue {
        make_val(CelValue::Int(n))
    }

    pub(super) unsafe fn make_array(elements: Vec<CelValue>) -> *mut CelValue {
        make_val(CelValue::Array(elements))
    }
}
