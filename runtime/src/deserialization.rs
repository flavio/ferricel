//! JSON and protobuf deserialization from WASM memory into CelValue objects.
//! Handles parsing bytes and allocating CelValue on the heap.

use crate::error::abort_with_error;
use crate::types::{CelMapKey, CelValue};
use ferricel_types::proto::Bindings;
use ferricel_types::proto::cel::expr::value::Kind;
use prost::Message;
use slog::{error, info};
use std::collections::HashMap;

/// Decode i64 into (ptr, len) tuple.
/// Low 32 bits = pointer, High 32 bits = length.
#[inline]
pub fn decode_ptr_len(encoded: i64) -> (i32, i32) {
    let ptr = (encoded & 0xFFFFFFFF) as i32;
    let len = (encoded >> 32) as i32;
    (ptr, len)
}

/// Deserialize JSON from WASM memory into a CelValue.
///
/// # Parameters
/// - `encoded`: i64 with ptr in low 32 bits, len in high 32 bits
///   - If 0, returns null pointer (no data provided)
///   - Otherwise, reads JSON bytes from memory and parses
///
/// # Returns
/// - Pointer to boxed CelValue on success
/// - Null pointer (0) if encoded is 0 or parsing fails
///
/// # Safety
/// - The returned pointer must be freed with `cel_free_value`
/// - Caller must ensure the memory region [ptr, ptr+len) is valid
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_deserialize_json(encoded: i64) -> *mut CelValue {
    let log = crate::logging::get_logger();

    // Handle null/absent input
    if encoded == 0 {
        info!(log, "Deserializing null input");
        return core::ptr::null_mut();
    }

    // Decode pointer and length
    let (ptr, len) = decode_ptr_len(encoded);

    // Validate length
    if len < 0 {
        error!(log, "Invalid length in encoded parameter";
            "function" => "cel_deserialize_json",
            "length" => len);
        abort_with_error("no such overload");
    }

    info!(log, "Deserializing JSON input";
        "ptr" => ptr,
        "len" => len);

    // Read JSON bytes from memory
    // SAFETY: Caller guarantees memory region [ptr, ptr+len) is valid
    let json_bytes = unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) };

    // Parse JSON into CelValue
    match serde_json::from_slice::<CelValue>(json_bytes) {
        Ok(value) => {
            // Box the value and return raw pointer
            let boxed = Box::new(value);
            Box::into_raw(boxed)
        }
        Err(err) => {
            // JSON parsing failed - panic with error message
            {
                error!(log, "Failed to parse JSON";
                "function" => "cel_deserialize_json",
                "error" => format!("{:?}", err));
                abort_with_error("no such overload")
            }
        }
    }
}

/// Deserialize a protobuf-encoded `Bindings` message from WASM memory into a
/// `CelValue::Object` map, preserving full type fidelity for all CEL types.
///
/// The wire format is a `ferricel.Bindings` protobuf message:
///   `message Bindings { map<string, cel.expr.Value> variables = 1; }`
///
/// This is the typed alternative to `cel_deserialize_json`, which loses type
/// information for `Bytes`, `UInt`, `Timestamp`, `Duration`, etc.
///
/// # Parameters
/// - `encoded`: i64 with ptr in low 32 bits, len in high 32 bits
///   - If 0, returns null pointer (no bindings provided)
///
/// # Returns
/// - Pointer to boxed `CelValue::Object` on success
/// - Null pointer (0) if encoded is 0
/// - Aborts on malformed proto input
///
/// # Safety
/// - The returned pointer must be freed with `cel_free_value`
/// - Caller must ensure the memory region [ptr, ptr+len) is valid
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_deserialize_proto(encoded: i64) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if encoded == 0 {
        info!(log, "Deserializing null proto input");
        return core::ptr::null_mut();
    }

    let (ptr, len) = decode_ptr_len(encoded);

    if len < 0 {
        error!(log, "Invalid length in encoded parameter";
            "function" => "cel_deserialize_proto",
            "length" => len);
        abort_with_error("no such overload");
    }

    info!(log, "Deserializing proto input";
        "ptr" => ptr,
        "len" => len);

    // SAFETY: Caller guarantees memory region [ptr, ptr+len) is valid
    let proto_bytes = unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) };

    // Decode the Bindings proto message
    let bindings = match Bindings::decode(proto_bytes) {
        Ok(b) => b,
        Err(err) => {
            error!(log, "Failed to decode proto Bindings";
                "function" => "cel_deserialize_proto",
                "error" => format!("{:?}", err));
            abort_with_error("no such overload");
        }
    };

    // Convert map<string, cel.expr.Value> → CelValue::Object
    let mut map: HashMap<CelMapKey, CelValue> = HashMap::new();
    for (name, cel_val) in bindings.variables {
        let value = proto_value_to_cel(cel_val);
        map.insert(CelMapKey::String(name), value);
    }

    let boxed = Box::new(CelValue::Object(map));
    Box::into_raw(boxed)
}

/// Convert a `cel.expr.Value` proto message into a `CelValue`.
fn proto_value_to_cel(v: ferricel_types::proto::cel::expr::Value) -> CelValue {
    match v.kind {
        None => CelValue::Null,
        Some(Kind::NullValue(_)) => CelValue::Null,
        Some(Kind::BoolValue(b)) => CelValue::Bool(b),
        Some(Kind::Int64Value(i)) => CelValue::Int(i),
        Some(Kind::Uint64Value(u)) => CelValue::UInt(u),
        Some(Kind::DoubleValue(d)) => CelValue::Double(d),
        Some(Kind::StringValue(s)) => CelValue::String(s),
        Some(Kind::BytesValue(b)) => CelValue::Bytes(b.into()),
        Some(Kind::TypeValue(t)) => CelValue::Type(t),
        Some(Kind::ListValue(list)) => {
            let arr = list.values.into_iter().map(proto_value_to_cel).collect();
            CelValue::Array(arr)
        }
        Some(Kind::MapValue(map)) => {
            let mut cel_map = HashMap::new();
            for entry in map.entries {
                if let (Some(k), Some(v)) = (entry.key, entry.value) {
                    let cel_key = proto_value_to_cel(k);
                    let cel_val = proto_value_to_cel(v);
                    if let Some(map_key) = CelMapKey::from_cel_value(&cel_key) {
                        cel_map.insert(map_key, cel_val);
                    }
                }
            }
            CelValue::Object(cel_map)
        }
        // EnumValue and ObjectValue (google.protobuf.Any) have no direct CelValue
        // equivalent — fall back to null to avoid a hard abort.
        Some(Kind::EnumValue(_)) | Some(Kind::ObjectValue(_)) => CelValue::Null,
    }
}

/// Free a CelValue that was allocated by `cel_deserialize_json` or `cel_deserialize_proto`.
///
/// # Safety
/// - `ptr` must be a valid pointer returned from `cel_deserialize_json`
/// - `ptr` must not be used after calling this function
/// - Calling with null pointer is safe (no-op)
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_free_value(ptr: *mut CelValue) {
    if !ptr.is_null() {
        // Reconstruct the Box and let it drop
        // SAFETY: ptr is valid and was created by cel_deserialize_json
        let _boxed = unsafe { Box::from_raw(ptr) };
        // Box is dropped here, freeing the memory
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CelMapKey;

    #[test]
    fn test_decode_ptr_len() {
        let encoded: i64 = (0x5678_i64 << 32) | 0x1234_i64;
        let (ptr, len) = decode_ptr_len(encoded);

        assert_eq!(ptr, 0x1234);
        assert_eq!(len, 0x5678);
    }

    #[test]
    fn test_decode_ptr_len_zero() {
        let (ptr, len) = decode_ptr_len(0);
        assert_eq!(ptr, 0);
        assert_eq!(len, 0);
    }

    #[test]
    fn test_deserialize_null_encoded() {
        unsafe {
            let result = cel_deserialize_json(0);
            assert!(result.is_null());
        }
    }

    // Test JSON parsing directly without involving pointer math
    #[test]
    fn test_parse_json_int() {
        let json = b"42";
        let value: CelValue = serde_json::from_slice(json).unwrap();
        match value {
            CelValue::Int(n) => assert_eq!(n, 42),
            _ => panic!("Expected Int, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_json_bool() {
        let json = b"true";
        let value: CelValue = serde_json::from_slice(json).unwrap();
        match value {
            CelValue::Bool(b) => assert!(b),
            _ => panic!("Expected Bool, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_json_double() {
        let json = b"3.15";
        let value: CelValue = serde_json::from_slice(json).unwrap();
        match value {
            CelValue::Double(d) => assert_eq!(d, 3.15),
            _ => panic!("Expected Double, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_json_string() {
        let json = b"\"hello\"";
        let value: CelValue = serde_json::from_slice(json).unwrap();
        match value {
            CelValue::String(s) => assert_eq!(s, "hello"),
            _ => panic!("Expected String, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_json_null() {
        let json = b"null";
        let value: CelValue = serde_json::from_slice(json).unwrap();
        match value {
            CelValue::Null => {}
            _ => panic!("Expected Null, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_json_array() {
        let json = b"[1,2,3]";
        let value: CelValue = serde_json::from_slice(json).unwrap();
        match value {
            CelValue::Array(arr) => {
                assert_eq!(arr.len(), 3);
                match (&arr[0], &arr[1], &arr[2]) {
                    (CelValue::Int(a), CelValue::Int(b), CelValue::Int(c)) => {
                        assert_eq!(*a, 1);
                        assert_eq!(*b, 2);
                        assert_eq!(*c, 3);
                    }
                    _ => panic!("Expected array of ints"),
                }
            }
            _ => panic!("Expected Array, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_json_object() {
        let json = b"{\"count\":42,\"enabled\":true}";
        let value: CelValue = serde_json::from_slice(json).unwrap();
        match value {
            CelValue::Object(map) => {
                assert_eq!(map.len(), 2);
                match map.get(&CelMapKey::String("count".into())) {
                    Some(CelValue::Int(n)) => assert_eq!(*n, 42),
                    _ => panic!("Expected count=42"),
                }
                match map.get(&CelMapKey::String("enabled".into())) {
                    Some(CelValue::Bool(b)) => assert!(*b),
                    _ => panic!("Expected enabled=true"),
                }
            }
            _ => panic!("Expected Object, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_json_nested_object() {
        let json = b"{\"user\":{\"name\":\"test\"},\"id\":123}";
        let value: CelValue = serde_json::from_slice(json).unwrap();
        match value {
            CelValue::Object(map) => {
                match map.get(&CelMapKey::String("user".into())) {
                    Some(CelValue::Object(user_map)) => {
                        match user_map.get(&CelMapKey::String("name".into())) {
                            Some(CelValue::String(s)) => assert_eq!(s, "test"),
                            _ => panic!("Expected user.name=test"),
                        }
                    }
                    _ => panic!("Expected user object"),
                }
                match map.get(&CelMapKey::String("id".into())) {
                    Some(CelValue::Int(n)) => assert_eq!(*n, 123),
                    _ => panic!("Expected id=123"),
                }
            }
            _ => panic!("Expected Object, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = b"not valid json{";
        let result = serde_json::from_slice::<CelValue>(json);
        assert!(result.is_err());
    }
}
