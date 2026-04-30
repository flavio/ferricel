//! ferricel-core: CEL to WASM compiler and runtime
//!
//! This crate provides the core functionality for compiling Common Expression Language (CEL)
//! expressions into WebAssembly modules and executing them.
//!
//! ## Features
//!
//! - **Compiler**: Compiles CEL expressions to standalone WASM modules
//! - **Runtime**: Executes WASM modules with variable bindings
//! - **Type Support**: Handles integers, unsigned integers, doubles, strings, booleans, lists, and maps
//! - **Extensions**: Host-provided functions callable from CEL expressions
//!
//! ## Example
//!
//! ```rust,ignore
//! use ferricel_core::{CelEngine, compiler::Builder};
//!
//! // Compile a CEL expression to WASM
//! let wasm_bytes = Builder::new().build().compile("x + y")?;
//!
//! // Execute the WASM module with variable bindings
//! let engine = CelEngine::new(logger);
//! let bindings = r#"{"x": 1, "y": 2}"#;
//! let result = engine.execute(&wasm_bytes, Some(bindings))?;
//! println!("Result: {}", result);
//! ```

pub mod compiler;
pub mod runtime;
pub mod schema;

// Re-export commonly used types for convenience
pub use compiler::{Compiler, ExtensionKey};
pub use runtime::CelEngine;
pub use schema::ProtoSchema;
