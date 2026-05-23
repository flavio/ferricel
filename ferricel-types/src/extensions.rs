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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

// ─── Builder chain declarations ───────────────────────────────────────────────

/// One step in a fluent builder chain extension (e.g. `kw.k8s`, `kw.sigstore`).
///
/// A builder chain lets the host expose CEL APIs that mirror the cel-go pattern
/// of opaque intermediate types accumulating state through method calls, with a
/// terminal call that actually invokes the host.
///
/// At runtime each intermediate object is represented as a `CelValue::Object`
/// (map) with a reserved `"__type__"` key holding the step's `output_type` tag.
/// This lets the runtime validate the receiver's type and lets the host
/// deserialize all accumulated fields from the map payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuilderStep {
    /// Global entry-point that starts a new chain.
    ///
    /// Example: `kw.k8s.apiVersion("v1")` where
    /// - `function`    = `"kw.k8s.apiVersion"`
    /// - `state_key`   = `"apiVersion"`
    /// - `output_type` = `"kw.k8s.ClientBuilder"`
    ///
    /// Produces a `CelValue::Object` tagged `{"__type__":"kw.k8s.ClientBuilder",
    /// "apiVersion":"v1"}`.
    Entry {
        /// Full dotted CEL function name, e.g. `"kw.k8s.apiVersion"`.
        function: String,
        /// JSON key under which the argument is stored in the state map.
        state_key: String,
        /// Type tag written to `"__type__"` in the output map.
        output_type: String,
    },

    /// Receiver-style chaining step that accumulates one value into the state map.
    ///
    /// Example: `.kind("Pod")` on a `kw.k8s.ClientBuilder` where
    /// - `function`    = `"kind"`
    /// - `input_type`  = `"kw.k8s.ClientBuilder"`
    /// - `state_key`   = `"kind"`
    /// - `output_type` = `"kw.k8s.Client"`
    /// - `accumulate`  = `false`
    ///
    /// When `accumulate` is `true` the new value is appended to an existing
    /// `CelValue::Array` under `state_key` (e.g. repeated `.fieldMask()` calls).
    Chain {
        /// Method name, e.g. `"kind"`.
        function: String,
        /// Expected `"__type__"` tag of the receiver (used for runtime validation).
        input_type: String,
        /// JSON key under which the argument is stored.
        state_key: String,
        /// Type tag written to `"__type__"` in the output map.
        output_type: String,
        /// When `true`, successive calls append to an array instead of overwriting.
        accumulate: bool,
    },

    /// Terminal step that emits a host extension call with the accumulated map.
    ///
    /// Example: `.list()` on a `kw.k8s.Client` where
    /// - `function`       = `"list"`
    /// - `input_type`     = `"kw.k8s.Client"`
    /// - `extra_arg_key`  = `None`
    /// - `host_namespace` = `"kw.k8s"`
    /// - `host_function`  = `"list"`
    ///
    /// For `.get("nginx")`: `extra_arg_key = Some("name")`.
    /// The extra argument (if any) is folded into the map before the host call.
    Terminal {
        /// Method name, e.g. `"list"`, `"get"`, `"verify"`.
        function: String,
        /// Expected `"__type__"` tag of the receiver.
        input_type: String,
        /// If `Some`, one extra positional argument is stored under this key before the call.
        extra_arg_key: Option<String>,
        /// Namespace field of `ExtensionCallPayload` sent to the host.
        host_namespace: String,
        /// Function field of `ExtensionCallPayload` sent to the host.
        host_function: String,
    },
}

impl BuilderStep {
    /// The CEL function name this step matches on.
    pub fn function(&self) -> &str {
        match self {
            Self::Entry { function, .. }
            | Self::Chain { function, .. }
            | Self::Terminal { function, .. } => function.as_str(),
        }
    }

    /// The `output_type` tag produced by Entry or Chain steps (`None` for Terminal).
    pub fn output_type(&self) -> Option<&str> {
        match self {
            Self::Entry { output_type, .. } | Self::Chain { output_type, .. } => {
                Some(output_type.as_str())
            }
            Self::Terminal { .. } => None,
        }
    }
}

/// Declaration of a complete fluent builder chain (e.g. `kw.k8s`, `kw.sigstore`).
///
/// Pass one `BuilderChainDecl` per library to
/// [`Builder::with_builder_chain`](ferricel_core::compiler::Builder::with_builder_chain).
///
/// # Example — declaring `kw.k8s`
///
/// ```rust
/// use ferricel_types::extensions::{BuilderChainDecl, BuilderStep};
///
/// let kw_k8s = BuilderChainDecl {
///     steps: vec![
///         BuilderStep::Entry {
///             function: "kw.k8s.apiVersion".into(),
///             state_key: "apiVersion".into(),
///             output_type: "kw.k8s.ClientBuilder".into(),
///         },
///         BuilderStep::Chain {
///             function: "kind".into(),
///             input_type: "kw.k8s.ClientBuilder".into(),
///             state_key: "kind".into(),
///             output_type: "kw.k8s.Client".into(),
///             accumulate: false,
///         },
///         BuilderStep::Terminal {
///             function: "list".into(),
///             input_type: "kw.k8s.Client".into(),
///             extra_arg_key: None,
///             host_namespace: "kw.k8s".into(),
///             host_function: "list".into(),
///         },
///     ],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuilderChainDecl {
    /// All steps — Entry, Chain, and Terminal — that form this library.
    pub steps: Vec<BuilderStep>,
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
