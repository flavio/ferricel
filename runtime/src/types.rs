//! CEL value type definitions for JSON serialization.

use serde::Serialize;

/// Represents a CEL value that can be serialized to JSON.
/// Uses untagged serialization for raw JSON output (e.g., `42` instead of `{"Int": 42}`).
#[derive(Serialize)]
#[serde(untagged)]
pub enum CelValue {
    Int(i64),
    Bool(bool),
    // Future: String(String), Double(f64), etc.
}
