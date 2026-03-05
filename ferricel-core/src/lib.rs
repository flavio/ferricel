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
//!
//! ## Example
//!
//! ```rust,ignore
//! use ferricel_core::{compile_cel_to_wasm, execute_wasm};
//! use ferricel_types::LogLevel;
//! use slog::Logger;
//!
//! // Compile a CEL expression to WASM
//! let wasm_bytes = compile_cel_to_wasm("x + y")?;
//!
//! // Execute the WASM module with variable bindings
//! let bindings = r#"{"x": 1, "y": 2}"#;
//! let result = execute_wasm(&wasm_bytes, Some(bindings), LogLevel::Error, logger)?;
//! println!("Result: {}", result); // {"result": 3}
//! ```

pub mod compiler;
pub mod runtime;
pub mod schema;

// Re-export commonly used functions for convenience
pub use compiler::{CompilerOptions, compile_cel_to_wasm};
pub use runtime::execute_wasm;
pub use schema::ProtoSchema;
