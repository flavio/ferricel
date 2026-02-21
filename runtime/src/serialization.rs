//! JSON serialization of CEL values with memory-encoded results.
//! Returns i64 with pointer in low 32 bits, length in high 32 bits.

use crate::memory::cel_malloc;
use crate::types::CelValue;

/// Encode pointer and length into a single i64.
/// Low 32 bits = pointer, High 32 bits = length.
#[inline]
pub(crate) fn encode_ptr_len(ptr: i32, len: i32) -> i64 {
    ((len as i64) << 32) | (ptr as i64 & 0xFFFFFFFF)
}

/// Serialize a CelValue to JSON and return (ptr, len) encoded in i64.
fn serialize_to_json(value: &CelValue) -> i64 {
    // Serialize to JSON bytes
    let json_bytes = serde_json::to_vec(value).expect("Failed to serialize CelValue to JSON");

    let len = json_bytes.len();

    // Allocate memory for the JSON
    let ptr = cel_malloc(len);

    // Copy JSON bytes to allocated memory
    unsafe {
        std::ptr::copy_nonoverlapping(json_bytes.as_ptr(), ptr, len);
    }

    // Encode and return
    encode_ptr_len(ptr as i32, len as i32)
}

/// Convert i64 result to CelValue::Int and serialize to JSON.
/// Returns encoded (ptr, len) as i64.
#[unsafe(no_mangle)]
pub extern "C" fn cel_serialize_int(value: i64) -> i64 {
    let cel_value = CelValue::Int(value);
    serialize_to_json(&cel_value)
}

/// Convert i64 boolean (0 or 1) to CelValue::Bool and serialize to JSON.
/// Returns encoded (ptr, len) as i64.
#[unsafe(no_mangle)]
pub extern "C" fn cel_serialize_bool(value: i64) -> i64 {
    let cel_value = CelValue::Bool(value != 0);
    serialize_to_json(&cel_value)
}
