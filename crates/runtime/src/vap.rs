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
/// - `message_ptr`: pointer to a `CelValue::String` containing the rejection
///   message. If null or not a string, a generic message is used.
/// - `code`: HTTP status code (e.g. 422).
///
/// # Returns
/// Packed i64 ptr+len pointing to the JSON bytes in Wasm linear memory.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_serialize_vap_reject(message_ptr: *mut CelValue, code: i32) -> i64 {
    let message: String = if message_ptr.is_null() {
        "validation failed".to_string()
    } else {
        match unsafe { &*message_ptr } {
            CelValue::String(s) => s.clone(),
            other => {
                serde_json::to_string(other).unwrap_or_else(|_| "validation failed".to_string())
            }
        }
    };

    let mut map = HashMap::new();
    map.insert(CelMapKey::from("accepted"), CelValue::Bool(false));
    map.insert(CelMapKey::from("message"), CelValue::String(message));
    map.insert(CelMapKey::from("code"), CelValue::Int(code as i64));
    crate::serialization::serialize_to_json(&CelValue::Object(map))
}
