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

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;

    use alloc::vec;
    use hashbrown::HashMap;

    /// Test that CelValue variants can be serialized to JSON (format verification only)
    /// Note: These tests verify JSON formatting but don't test WASM memory allocation
    #[test]
    fn test_celvalue_json_int() {
        let value = CelValue::Int(42);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "42");
    }

    #[test]
    fn test_celvalue_json_negative_int() {
        let value = CelValue::Int(-100);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "-100");
    }

    #[test]
    fn test_celvalue_json_bool_true() {
        let value = CelValue::Bool(true);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "true");
    }

    #[test]
    fn test_celvalue_json_bool_false() {
        let value = CelValue::Bool(false);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "false");
    }

    #[test]
    fn test_celvalue_json_double() {
        let value = CelValue::Double(3.14);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "3.14");
    }

    #[test]
    fn test_celvalue_json_string() {
        let value = CelValue::String("hello".into());
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, r#""hello""#);
    }

    #[test]
    fn test_celvalue_json_null() {
        let value = CelValue::Null;
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "null");
    }

    #[test]
    fn test_celvalue_json_array() {
        let value = CelValue::Array(vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3)]);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "[1,2,3]");
    }

    #[test]
    fn test_celvalue_json_empty_array() {
        let value = CelValue::Array(vec![]);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_celvalue_json_object() {
        let mut map = HashMap::default();
        map.insert("count".into(), CelValue::Int(42));
        map.insert("enabled".into(), CelValue::Bool(true));

        let value = CelValue::Object(map);
        let json = serde_json::to_string(&value).unwrap();

        // JSON object key order is not guaranteed, so parse and check
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Invalid JSON");
        assert_eq!(parsed["count"], 42);
        assert_eq!(parsed["enabled"], true);
    }

    #[test]
    fn test_celvalue_json_nested_object() {
        let mut inner = HashMap::default();
        inner.insert("name".into(), CelValue::String("test".into()));

        let mut outer = HashMap::default();
        outer.insert("user".into(), CelValue::Object(inner));
        outer.insert("id".into(), CelValue::Int(123));

        let value = CelValue::Object(outer);
        let json = serde_json::to_string(&value).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Invalid JSON");
        assert_eq!(parsed["user"]["name"], "test");
        assert_eq!(parsed["id"], 123);
    }

    #[test]
    fn test_encode_ptr_len() {
        let encoded = encode_ptr_len(0x1234, 0x5678);

        // Check decoding
        let ptr = (encoded & 0xFFFFFFFF) as i32;
        let len = (encoded >> 32) as i32;

        assert_eq!(ptr, 0x1234);
        assert_eq!(len, 0x5678);
    }
}
