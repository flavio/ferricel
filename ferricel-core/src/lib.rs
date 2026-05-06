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

pub mod compiler;
pub mod runtime;
pub mod schema;

// Re-export commonly used types for convenience
pub use compiler::{Compiler, ExtensionKey};
pub use runtime::Engine;
pub use schema::ProtoSchema;
