//! ferricel-core: CEL to Wasm compiler and runtime
//!
//! This crate provides the core functionality for compiling Common Expression Language (CEL)
//! expressions into WebAssembly modules and executing them.
//!
//! ## Features
//!
//! - **Compiler**: Compiles CEL expressions to standalone Wasm modules
//! - **Runtime**: Executes Wasm modules with variable bindings
//! - **Type Support**: Handles integers, unsigned integers, doubles, strings, booleans, lists, and maps
//! - **Extensions**: Host-provided functions callable from CEL expressions
//!
//! ## Example
//!
//! ```rust,ignore
//! use ferricel_core::{compiler, runtime};
//!
//! // Compile a CEL expression to Wasm
//! let wasm_bytes = compiler::Builder::new().build().compile("x + y")?;
//!
//! // Execute the Wasm module with variable bindings
//! let bindings = r#"{"x": 1, "y": 2}"#;
//! let result = runtime::Builder::new()
//!     .with_logger(logger)
//!     .build()
//!     .execute(&wasm_bytes, Some(bindings))?;
//! println!("Result: {}", result);
//! ```

pub mod compiler;
pub mod runtime;
pub mod schema;

// Re-export commonly used types for convenience
pub use compiler::{Compiler, ExtensionKey};
pub use runtime::Engine;
pub use schema::ProtoSchema;
