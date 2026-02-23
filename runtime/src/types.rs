//! CEL value type definitions for JSON serialization and deserialization.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

/// Type alias for HashMap with ahash for better no_std compatibility
type CelMap = HashMap<String, CelValue, hashbrown::hash_map::DefaultHashBuilder>;

/// Represents a CEL value that can be serialized to/from JSON.
/// Uses untagged serialization for raw JSON output (e.g., `42` instead of `{"Int": 42}`).
///
/// Supports all JSON types:
/// - Primitives: Int, Bool, Double, String
/// - Collections: Array, Object
/// - Special: Null
///
/// Note: Bytes type is not yet supported due to ser ialization complexity.
/// It will be added in a future update.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CelValue {
    /// Null value (checked first to avoid ambiguity)
    Null,

    /// Boolean value
    Bool(bool),

    /// 64-bit signed integer
    Int(i64),

    /// 64-bit floating point number
    Double(f64),

    /// UTF-8 string
    String(String),

    /// Array of CelValues
    Array(Vec<CelValue>),

    /// Object/map with string keys
    Object(CelMap),
}
