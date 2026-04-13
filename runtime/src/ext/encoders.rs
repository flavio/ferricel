//! CEL encoder extension library functions.
//!
//! Implements `base64.encode(bytes) -> string` and `base64.decode(string) -> bytes`.

use base64::{Engine as _, engine::general_purpose};

use crate::types::CelValue;

/// `base64.encode(b) -> string` — encodes bytes to a standard base64 string (with padding).
///
/// # Safety
///
/// Caller must ensure the pointer argument points to a valid `CelValue` instance
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_base64_encode(bytes_ptr: *const CelValue) -> *mut CelValue {
    let bytes = match unsafe { &*bytes_ptr } {
        CelValue::Bytes(b) => b,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "base64.encode: argument is not bytes".to_string(),
            )));
        }
    };
    let encoded = general_purpose::STANDARD.encode(bytes);
    Box::into_raw(Box::new(CelValue::String(encoded)))
}

/// `base64.decode(s) -> bytes` — decodes a base64 string to bytes.
///
/// Accepts both padded (`aGVsbG8=`) and unpadded (`aGVsbG8`) base64.
/// Returns an error value if the string is not valid base64.
///
/// # Safety
///
/// Caller must ensure the pointer argument points to a valid `CelValue` instance
/// allocated by the WASM host.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_base64_decode(string_ptr: *const CelValue) -> *mut CelValue {
    let s = match unsafe { &*string_ptr } {
        CelValue::String(s) => s,
        _ => {
            return Box::into_raw(Box::new(CelValue::Error(
                "base64.decode: argument is not a string".to_string(),
            )));
        }
    };
    // Try padded standard encoding first, then fall back to unpadded.
    let result = general_purpose::STANDARD
        .decode(s.as_bytes())
        .or_else(|_| general_purpose::STANDARD_NO_PAD.decode(s.as_bytes()));

    match result {
        Ok(bytes) => Box::into_raw(Box::new(CelValue::Bytes(bytes))),
        Err(e) => Box::into_raw(Box::new(CelValue::Error(format!("base64.decode: {}", e)))),
    }
}
