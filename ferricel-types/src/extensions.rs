//! Extension function types for host-provided CEL extensions.
//!
//! Extensions allow CEL programs to call functions implemented by the host.
//! They are declared at compile time (for validation) and implemented at runtime.

use serde::{Deserialize, Serialize};

/// Compile-time declaration of a host-provided extension function.
///
/// An extension declaration tells the compiler:
/// - What namespace and function name it responds to
/// - Whether it can be called as `x.func()` (receiver-style)
/// - Whether it can be called as `func(x)` or `ns.func(x)` (global-style)
/// - How many total arguments (including receiver if receiver-style) it expects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionDecl {
    /// Namespace prefix, e.g. `Some("math")` for `math.abs()`. `None` for flat names.
    pub namespace: Option<String>,
    /// Function name, e.g. `"abs"`, `"reverse"`.
    pub function: String,
    /// Whether the extension can be called as `x.func()`.
    pub receiver_style: bool,
    /// Whether the extension can be called as `func(x)` or `ns.func(x)`.
    pub global_style: bool,
    /// Total number of arguments the host receives in the `args` array.
    /// For receiver-style calls, the receiver counts as one argument.
    pub num_args: usize,
}

/// Wire format payload sent from the Wasm guest to the host when calling an extension.
///
/// Serialized as JSON and passed through the `cel_call_extension` host import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionCallPayload {
    /// Namespace of the function, or `null` for non-namespaced functions.
    pub namespace: Option<String>,
    /// Name of the function to call.
    pub function: String,
    /// Serialized arguments (same JSON representation as `cel_serialize_value` produces).
    /// For receiver-style calls, the receiver is always `args[0]`.
    pub args: Vec<serde_json::Value>,
}
