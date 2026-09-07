//! ferricel-core: CEL to Wasm compiler and runtime
//!
//! This crate provides the core functionality for compiling Common Expression Language (CEL)
//! expressions into WebAssembly modules and executing them.
//!
//! User guide: <https://flavio.github.io/ferricel/>
//!
//! ## Capabilities
//!
//! - **Compiler**: Compiles CEL expressions to standalone Wasm modules
//! - **Runtime**: Executes Wasm modules with variable bindings
//! - **Type Support**: Handles integers, unsigned integers, doubles, strings, booleans, lists, and maps
//! - **Extensions**: Host-provided functions callable from CEL expressions
//!
//! ## Crate Features
//! - **`k8s-vap`** *(default)*: Compiles Kubernetes
//!   [`ValidatingAdmissionPolicy`](https://kubernetes.io/docs/reference/access-authn-authz/admission-controllers/#validatingadmissionpolicy)
//!   YAML to Wasm
//!
//! ## Example
//!
//! ```rust
//! use ferricel_core::{compiler, runtime};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Compile a CEL expression to Wasm
//! let wasm_bytes = compiler::Builder::new().build().compile("x + y")?;
//!
//! // Execute the Wasm module with variable bindings
//! let bindings = r#"{"x": 1, "y": 2}"#;
//! let result = runtime::Builder::new()
//!     .with_wasm(wasm_bytes)
//!     .build()?
//!     .eval(Some(bindings))?;
//! println!("Result: {}", result);
//! # Ok(())
//! # }
//! ```
//!
//! ## Runtime errors
//!
//! When the CEL expression produces a runtime error (divide by zero, an
//! unbound variable, a failed host extension), `eval` returns an error that
//! downcasts to [`CelRuntimeError`]. Other failures do not downcast to it.
//!
//! ```rust
//! use ferricel_core::{compiler, runtime, CelRuntimeError};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let wasm_bytes = compiler::Builder::new().build().compile("1 / x")?;
//! let engine = runtime::Builder::new().with_wasm(wasm_bytes).build()?;
//!
//! let err = engine.eval(Some(r#"{"x": 0}"#)).unwrap_err();
//! let cel_err = err.downcast_ref::<CelRuntimeError>().expect("a CEL runtime error");
//! assert_eq!(cel_err.message, "divide by zero");
//! assert!(cel_err.origin.is_none(), "no host extension was involved");
//! # Ok(())
//! # }
//! ```
//!
//! [`CelRuntimeError::origin`] is `Some` when a host extension produced the
//! error. It names the extension (`namespace` and `function`).

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod compiler;
pub mod inspect;
pub mod runtime;
pub mod schema;

// Re-export commonly used types for convenience
pub use compiler::{Compiler, ExtensionKey, extensions_used};
#[cfg(feature = "k8s-vap")]
#[cfg_attr(docsrs, doc(cfg(feature = "k8s-vap")))]
pub use compiler::{WELL_KNOWN_VAP_VARIABLES, vap_variables_used};
pub use ferricel_types::extensions::UsedExtension;
pub use inspect::{ModuleInfo, ProducerField, ProducerValue, inspect};
pub use runtime::{
    CelRuntimeError, Engine, EnginePre, Extension, ExtensionFn, ExtensionOrigin, Extensions,
    ResourceLimits,
};
pub use schema::ProtoSchema;
