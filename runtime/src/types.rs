//! CEL value type definitions for JSON serialization and deserialization.

use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashMap;
use url::Url;

/// Map key types allowed by CEL specification.
/// Per CEL spec, only boolean, int, uint, and string values can be map keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CelMapKey {
    Bool(bool),
    Int(i64),
    UInt(u64),
    String(String),
}

impl CelMapKey {
    /// Convert the key to a string representation for JSON serialization.
    pub fn to_string_key(&self) -> String {
        match self {
            CelMapKey::Bool(b) => b.to_string(),
            CelMapKey::Int(i) => i.to_string(),
            CelMapKey::UInt(u) => u.to_string(),
            CelMapKey::String(s) => s.clone(),
        }
    }

    /// Create a CelMapKey from a CelValue (if it's a valid key type).
    pub fn from_cel_value(value: &CelValue) -> Option<CelMapKey> {
        match value {
            CelValue::Bool(b) => Some(CelMapKey::Bool(*b)),
            CelValue::Int(i) => Some(CelMapKey::Int(*i)),
            CelValue::UInt(u) => Some(CelMapKey::UInt(*u)),
            CelValue::String(s) => Some(CelMapKey::String(s.clone())),
            _ => None,
        }
    }
}

impl From<String> for CelMapKey {
    fn from(s: String) -> Self {
        CelMapKey::String(s)
    }
}

impl From<&str> for CelMapKey {
    fn from(s: &str) -> Self {
        CelMapKey::String(s.to_string())
    }
}

/// Type alias for CEL maps with heterogeneous key types
type CelMap = HashMap<CelMapKey, CelValue>;

/// Helper module for deserializing maps with string keys from JSON
mod cel_map_serde {
    use super::*;
    use serde::{Deserialize, Deserializer};
    use std::collections::HashMap;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<CelMapKey, CelValue>, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize as a string-keyed HashMap first
        let string_map: HashMap<String, CelValue> = HashMap::deserialize(deserializer)?;
        // Convert all string keys to CelMapKey::String
        Ok(string_map
            .into_iter()
            .map(|(k, v)| (CelMapKey::String(k), v))
            .collect())
    }
}

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

    /// Object/map with heterogeneous key types (bool, int, uint, string)
    /// Deserialized from JSON with string keys only (JSON limitation)
    #[serde(deserialize_with = "cel_map_serde::deserialize")]
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

    /// Type - represents a CEL type value
    /// Used by the type() function and type denotations (e.g., int, bool, string)
    /// Serializes as a type_value object per CEL spec
    #[serde(skip_deserializing)]
    Type(String),

    /// Error - represents a runtime error that occurred during evaluation
    /// Errors can be propagated through the expression tree and potentially absorbed
    /// by short-circuit operators like && and ||
    /// Serializes as an error message string
    #[serde(skip_deserializing)]
    Error(String),

    /// URL - represents a parsed URL value created by the `url()` CEL function.
    /// Stores the parsed `url::Url` and the original input string.
    /// The original string is needed because the `url` crate normalises paths
    /// (e.g. `https://example.com` → path `"/"`) and we need to distinguish an
    /// explicitly-absent path from an explicit `"/"` path.
    /// Supports the Kubernetes CEL URL library:
    /// `getScheme()`, `getHost()`, `getHostname()`, `getPort()`,
    /// `getEscapedPath()`, `getQuery()`.
    /// Serializes as the URL string. Cannot be deserialized from JSON.
    #[serde(skip_deserializing)]
    Url(Url, String),
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
            CelValue::Double(d) => {
                // Handle special float values that JSON doesn't support natively
                if d.is_infinite() {
                    if d.is_sign_positive() {
                        serializer.serialize_str("Infinity")
                    } else {
                        serializer.serialize_str("-Infinity")
                    }
                } else if d.is_nan() {
                    serializer.serialize_str("NaN")
                } else {
                    serializer.serialize_f64(*d)
                }
            }
            CelValue::String(s) => serializer.serialize_str(s),
            CelValue::Bytes(bytes) => {
                use base64::{Engine as _, engine::general_purpose};
                let encoded = general_purpose::STANDARD.encode(bytes);
                serializer.serialize_str(&encoded)
            }
            CelValue::Array(arr) => arr.serialize(serializer),
            CelValue::Object(obj) => {
                // Convert CelMapKey to string keys for JSON serialization
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(obj.len()))?;
                for (key, value) in obj {
                    map.serialize_entry(&key.to_string_key(), value)?;
                }
                map.end()
            }
            CelValue::Timestamp(dt) => {
                // Use "Z" suffix for UTC timestamps instead of "+00:00" for CEL compliance
                let formatted = if dt.offset().local_minus_utc() == 0 {
                    dt.format("%Y-%m-%dT%H:%M:%S%.fZ").to_string()
                } else {
                    dt.to_rfc3339()
                };
                serializer.serialize_str(&formatted)
            }
            CelValue::Duration(d) => {
                let formatted = crate::chrono_helpers::format_duration(d);
                serializer.serialize_str(&formatted)
            }
            CelValue::Type(type_name) => {
                // Serialize as {"type_value": "type_name"} per CEL spec
                use serde::ser::SerializeStruct;
                let mut state = serializer.serialize_struct("Type", 1)?;
                state.serialize_field("type_value", type_name)?;
                state.end()
            }
            CelValue::Error(msg) => {
                // Serialize as {"error": "message"} to indicate this is an error value
                use serde::ser::SerializeStruct;
                let mut state = serializer.serialize_struct("Error", 1)?;
                state.serialize_field("error", msg)?;
                state.end()
            }
            CelValue::Url(u, _original) => serializer.serialize_str(u.as_str()),
        }
    }
}
