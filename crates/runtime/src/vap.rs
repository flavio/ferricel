//! VAP (ValidatingAdmissionPolicy) response serialization.
//!
//! These functions produce a Kubewarden-compatible `ValidationResponse` JSON
//! object (`{"accepted": true}` or `{"accepted": false, "message": "...", "code": N}`)
//! and return the result as a packed ptr+len `i64` — the same encoding used by
//! `cel_serialize_result`.

use std::collections::HashMap;

use crate::types::{CelMapKey, CelValue};

/// Serialize an acceptance response: `{"accepted":true}`.
///
/// # Returns
/// Packed i64 with ptr (low 32 bits) and len (high 32 bits) pointing to the
/// JSON bytes in Wasm linear memory.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_serialize_vap_accept() -> i64 {
    let mut map = HashMap::new();
    map.insert(CelMapKey::from("accepted"), CelValue::Bool(true));
    crate::serialization::serialize_to_json(&CelValue::Object(map))
}

/// Serialize a rejection response:
/// `{"accepted":false,"message":"<msg>","code":<code>}`.
///
/// # Parameters
/// - `message_ptr`: pointer to the result of the validation's
///   `messageExpression`, or null if none was specified. Used only if it is a
///   `CelValue::String`; per the Kubernetes VAP spec, a `messageExpression`
///   that errors or produces a non-string value falls back to the static
///   message.
/// - `fallback_ptr`: pointer to a `CelValue::String` containing the static
///   message (the validation's `message` field, or the default derived from the
///   expression text). Used when `message_ptr` is unusable.
/// - `code`: HTTP status code (e.g. 422).
///
/// # Returns
/// Packed i64 ptr+len pointing to the JSON bytes in Wasm linear memory.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_serialize_vap_reject(
    message_ptr: *mut CelValue,
    fallback_ptr: *mut CelValue,
    code: i32,
) -> i64 {
    let message = unsafe { resolve_reject_message(message_ptr, fallback_ptr) };

    let mut map = HashMap::new();
    map.insert(CelMapKey::from("accepted"), CelValue::Bool(false));
    map.insert(CelMapKey::from("message"), CelValue::String(message));
    map.insert(CelMapKey::from("code"), CelValue::Int(code as i64));
    crate::serialization::serialize_to_json(&CelValue::Object(map))
}

/// Pick the rejection message: the `messageExpression` result if it is a
/// string, else the static fallback if it is a string, else a generic message.
///
/// # Safety
///
/// Both pointers must be null or valid `CelValue` pointers.
unsafe fn resolve_reject_message(
    message_ptr: *mut CelValue,
    fallback_ptr: *mut CelValue,
) -> String {
    let as_string = |ptr: *mut CelValue| -> Option<String> {
        if ptr.is_null() {
            return None;
        }
        match unsafe { &*ptr } {
            CelValue::String(s) => Some(s.clone()),
            _ => None,
        }
    };

    as_string(message_ptr)
        .or_else(|| as_string(fallback_ptr))
        .unwrap_or_else(|| "validation failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{make_int, make_str, make_val};

    #[test]
    fn reject_message_prefers_message_expression_string() {
        let msg = unsafe { resolve_reject_message(make_str("from expr"), make_str("static")) };
        assert_eq!(msg, "from expr");
    }

    #[test]
    fn reject_message_falls_back_when_message_expression_is_null() {
        let msg = unsafe { resolve_reject_message(std::ptr::null_mut(), make_str("static")) };
        assert_eq!(msg, "static");
    }

    #[test]
    fn reject_message_falls_back_when_message_expression_is_error() {
        let err = make_val(CelValue::Error("division by zero".into()));
        let msg = unsafe { resolve_reject_message(err, make_str("static")) };
        assert_eq!(msg, "static");
    }

    #[test]
    fn reject_message_falls_back_when_message_expression_is_not_a_string() {
        let msg = unsafe { resolve_reject_message(make_int(42), make_str("static")) };
        assert_eq!(msg, "static");
    }

    #[test]
    fn reject_message_generic_when_nothing_usable() {
        let msg = unsafe { resolve_reject_message(std::ptr::null_mut(), std::ptr::null_mut()) };
        assert_eq!(msg, "validation failed");
        let msg = unsafe { resolve_reject_message(make_int(1), make_val(CelValue::Null)) };
        assert_eq!(msg, "validation failed");
    }
}
