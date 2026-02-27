//! CEL value type definitions for JSON serialization and deserialization.

use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashMap;

/// Type alias for HashMap with String keys
type CelMap = HashMap<String, CelValue>;

/// Represents a CEL value that can be serialized to/from JSON.
/// Uses untagged serialization for raw JSON output (e.g., `42` instead of `{"Int": 42}`).
///
/// Supports all JSON types:
/// - Primitives: Int, UInt, Bool, Double, String, Bytes
/// - Collections: Array, Object
/// - Temporal: Timestamp, Duration
/// - Special: Null
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum CelValue {
    /// Null value (checked first to avoid ambiguity)
    Null,

    /// Boolean value
    Bool(bool),

    /// 64-bit signed integer
    Int(i64),

    /// 64-bit unsigned integer
    /// Note: Cannot be deserialized from JSON. UInt values are created by the AST
    /// when parsing CEL literals like "100u". JSON numbers always deserialize to Int.
    #[serde(skip_deserializing)]
    UInt(u64),

    /// 64-bit floating point number
    Double(f64),

    /// UTF-8 string
    String(String),

    /// Bytes - arbitrary sequence of octets
    /// Serializes to base64-encoded string per CEL specification
    /// Note: Cannot be deserialized from JSON. Created via bytes literals or bytes() function.
    #[serde(skip_deserializing)]
    Bytes(Vec<u8>),

    /// Array of CelValues
    Array(Vec<CelValue>),

    /// Object/map with string keys
    Object(CelMap),

    /// Timestamp - google.protobuf.Timestamp
    /// Represents an absolute point in time with timezone
    /// Uses chrono::DateTime<FixedOffset> for RFC3339 compatibility
    /// Valid range: 0001-01-01T00:00:00Z to 9999-12-31T23:59:59.999999999Z
    /// Serializes to RFC3339 string format
    /// Note: Cannot be deserialized from JSON. Created via timestamp() CEL function.
    #[serde(skip_deserializing)]
    Timestamp(chrono::DateTime<chrono::FixedOffset>),

    /// Duration - google.protobuf.Duration
    /// Represents a signed, fixed-length span of time
    /// Uses chrono::Duration for CEL duration format compatibility
    /// Can be negative (for time going backwards)
    /// Serializes to duration string format (e.g., "1h30m", "1.5s")
    /// Note: Cannot be deserialized from JSON. Created via duration() CEL function.
    #[serde(skip_deserializing)]
    Duration(chrono::Duration),
}

// Custom serialization for CelValue to provide untagged JSON output
// with special formatting for Timestamp and Duration types.
impl Serialize for CelValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            CelValue::Null => serializer.serialize_none(),
            CelValue::Bool(b) => serializer.serialize_bool(*b),
            CelValue::Int(i) => serializer.serialize_i64(*i),
            CelValue::UInt(u) => serializer.serialize_u64(*u),
            CelValue::Double(d) => serializer.serialize_f64(*d),
            CelValue::String(s) => serializer.serialize_str(s),
            CelValue::Bytes(bytes) => {
                use base64::{Engine as _, engine::general_purpose};
                let encoded = general_purpose::STANDARD.encode(bytes);
                serializer.serialize_str(&encoded)
            }
            CelValue::Array(arr) => arr.serialize(serializer),
            CelValue::Object(obj) => obj.serialize(serializer),
            CelValue::Timestamp(dt) => serializer.serialize_str(&dt.to_rfc3339()),
            CelValue::Duration(d) => {
                let formatted = crate::chrono_helpers::format_duration(d);
                serializer.serialize_str(&formatted)
            }
        }
    }
}
