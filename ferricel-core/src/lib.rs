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
//! use ferricel_core::{compile_cel_to_wasm, execute_wasm_with_vars};
//! use ferricel_types::LogLevel;
//! use slog::Logger;
//!
//! // Compile a CEL expression to WASM
//! let wasm_bytes = compile_cel_to_wasm("1 + 1")?;
//!
//! // Execute the WASM module
//! let result = execute_wasm_with_vars(&wasm_bytes, None, None, LogLevel::Error, logger)?;
//! println!("Result: {}", result); // {"result": 2}
//! ```

pub mod compiler;
pub mod runtime;

// Re-export commonly used functions for convenience
pub use compiler::compile_cel_to_wasm;
pub use runtime::execute_wasm_with_vars;

/// Convenience function that compiles and executes a CEL expression in one step.
///
/// This is useful for quick evaluation without needing to manage WASM bytes.
///
/// # Arguments
///
/// * `expr` - The CEL expression to evaluate
/// * `input` - Optional JSON string containing input variables
/// * `data` - Optional JSON string containing data variables
/// * `log_level` - Logging level for execution
/// * `logger` - slog Logger instance
///
/// # Returns
///
/// JSON string with the result, e.g. `{"result": 42}`
///
/// # Example
///
/// ```rust,ignore
/// use ferricel_core::evaluate_cel;
/// use ferricel_types::LogLevel;
///
/// let result = evaluate_cel(
///     "input.x + input.y",
///     Some(r#"{"x": 10, "y": 20}"#),
///     None,
///     LogLevel::Error,
///     logger
/// )?;
/// ```
pub fn evaluate_cel(
    expr: &str,
    input: Option<&str>,
    data: Option<&str>,
    log_level: ferricel_types::LogLevel,
    logger: slog::Logger,
) -> Result<String, anyhow::Error> {
    let wasm_bytes = compile_cel_to_wasm(expr)?;
    execute_wasm_with_vars(&wasm_bytes, input, data, log_level, logger)
}
