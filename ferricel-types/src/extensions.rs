//! Extension function types for host-provided CEL extensions.
//!
//! Extensions allow CEL programs to call functions implemented by the host.
//! They are declared at compile time (for validation) and implemented at runtime.
//!
//! This module provides the type definitions. See the
//! [Host Extensions](https://flavio.github.io/ferricel/host-extensions.html)
//! chapter of the user guide for a tutorial with worked examples.

use serde::{Deserialize, Serialize};

/// Compile-time declaration of a host-provided extension function.
///
/// Tells the compiler the function's namespace, name, calling style, and arity.
///
/// See the [Flat Extensions](https://flavio.github.io/ferricel/host-extensions.html#flat-extensions)
/// section of the user guide for usage examples.
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
/// At runtime each intermediate object is a `CelValue::Object` (map) tagged
/// with a `"__type__"` key. The compiler uses `input_type` / `output_type`
/// and argument count to disambiguate steps registered under the same method
/// name.
///
/// See the [Builder Chains](https://flavio.github.io/ferricel/host-extensions.html#builder-chains)
/// section of the user guide for a tutorial covering multi-arg steps,
/// `MapEntry`, type disambiguation, and arity overloads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuilderStep {
    /// Global entry-point that starts a new chain (e.g. `kw.k8s.apiVersion("v1")`).
    ///
    /// Creates a fresh state map tagged with `output_type`.
    Entry {
        /// Full dotted CEL function name, e.g. `"kw.k8s.apiVersion"`.
        function: String,
        /// JSON keys under which each positional argument is stored in the state
        /// map. The number of keys determines the expected arity.
        state_keys: Vec<String>,
        /// Type tag written to `"__type__"` in the output map.
        output_type: String,
    },

    /// Receiver-style chaining step that stores positional arguments under
    /// fixed keys (e.g. `.kind("Pod")` or `.keyless("issuer", "subject")`).
    ///
    /// When `accumulate` is `true`, values are appended to an array instead
    /// of overwriting (e.g. repeated `.fieldMask()` calls).
    Chain {
        /// Method name, e.g. `"kind"`.
        function: String,
        /// Expected `"__type__"` tag of the receiver (used for compile-time
        /// disambiguation and runtime validation).
        input_type: String,
        /// JSON keys under which each positional argument is stored.
        /// The number of keys determines the expected arity.
        state_keys: Vec<String>,
        /// Type tag written to `"__type__"` in the output map.
        output_type: String,
        /// When `true`, successive calls append to an array instead of overwriting.
        accumulate: bool,
    },

    /// Receiver-style step that inserts a runtime key/value pair into a nested
    /// map (e.g. `.annotation("env", "prod")`). Always takes 2 arguments.
    /// Repeated calls merge into the same nested map.
    MapEntry {
        /// Method name, e.g. `"annotation"`.
        function: String,
        /// Expected `"__type__"` tag of the receiver.
        input_type: String,
        /// JSON key of the nested map in the state object, e.g. `"annotations"`.
        state_key: String,
        /// Type tag written to `"__type__"` in the output map.
        output_type: String,
    },

    /// Terminal step that invokes the host with the accumulated state map.
    ///
    /// Extra positional arguments (if any) are folded into the map before the
    /// host call.
    Terminal {
        /// Method name, e.g. `"list"`, `"get"`, `"verify"`.
        function: String,
        /// Expected `"__type__"` tag of the receiver.
        input_type: String,
        /// JSON keys for extra positional arguments folded into the map before
        /// calling the host. Empty for zero-arg terminals like `.list()`.
        extra_arg_keys: Vec<String>,
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
            | Self::MapEntry { function, .. }
            | Self::Terminal { function, .. } => function.as_str(),
        }
    }

    /// The `output_type` tag produced by Entry, Chain, or MapEntry steps
    /// (`None` for Terminal — terminals return a host `dyn` value).
    pub fn output_type(&self) -> Option<&str> {
        match self {
            Self::Entry { output_type, .. }
            | Self::Chain { output_type, .. }
            | Self::MapEntry { output_type, .. } => Some(output_type.as_str()),
            Self::Terminal { .. } => None,
        }
    }

    /// The `input_type` tag expected on the receiver (Chain, MapEntry, Terminal).
    /// Entry steps have no receiver, so this returns `None` for them.
    pub fn input_type(&self) -> Option<&str> {
        match self {
            Self::Entry { .. } => None,
            Self::Chain { input_type, .. }
            | Self::MapEntry { input_type, .. }
            | Self::Terminal { input_type, .. } => Some(input_type.as_str()),
        }
    }

    /// The number of positional arguments this step expects (excluding the receiver).
    pub fn expected_args(&self) -> usize {
        match self {
            Self::Entry { state_keys, .. } => state_keys.len(),
            Self::Chain { state_keys, .. } => state_keys.len(),
            Self::MapEntry { .. } => 2,
            Self::Terminal { extra_arg_keys, .. } => extra_arg_keys.len(),
        }
    }
}

/// Declaration of a complete fluent builder chain (e.g. `kw.k8s`, `kw.sigstore`).
///
/// Pass one `BuilderChainDecl` per library to
/// [`ferricel_core::compiler::Builder::with_builder_chain`](https://docs.rs/ferricel-core/latest/ferricel_core/compiler/struct.Builder.html#method.with_builder_chain).
///
/// See the [Builder Chains](https://flavio.github.io/ferricel/host-extensions.html#builder-chains)
/// section of the user guide for a full tutorial.
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
///             state_keys: vec!["apiVersion".into()],
///             output_type: "kw.k8s.ClientBuilder".into(),
///         },
///         BuilderStep::Chain {
///             function: "kind".into(),
///             input_type: "kw.k8s.ClientBuilder".into(),
///             state_keys: vec!["kind".into()],
///             output_type: "kw.k8s.Client".into(),
///             accumulate: false,
///         },
///         BuilderStep::Terminal {
///             function: "list".into(),
///             input_type: "kw.k8s.Client".into(),
///             extra_arg_keys: vec![],
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

/// One entry in the `ferricel.extensions` custom section.
///
/// Records a host extension that the compiled Wasm module may call at
/// evaluation time. The list is embedded at compile time and can be read back
/// with `ferricel_core::extensions_used`.
///
/// See the [Inspecting used extensions](https://flavio.github.io/ferricel/host-extensions.html#inspecting-used-extensions)
/// section of the user guide.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UsedExtension {
    /// Namespace of the extension, or `None` for flat (non-namespaced) functions.
    pub namespace: Option<String>,
    /// Function name.
    pub function: String,
}
