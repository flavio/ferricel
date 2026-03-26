// Shared test helpers for ferricel-core integration tests.
//
// These helpers are used across multiple test files. Adding `mod common;` and
// `use common::*;` (or individual imports) to any test file gives it access to
// all helpers here.
//
// # Test file map
//
// Each logical group of tests lives in its own file:
//
//   compiler_tests.rs      — WASM magic number, invalid expression (2 tests)
//   arithmetic_tests.rs    — Integer literals, arithmetic operators, comparisons, logical ops
//   double_tests.rs        — Double literals, arithmetic, division-by-zero, comparisons, type safety
//   json_output_tests.rs   — JSON serialization of integers, booleans, arithmetic results
//   list_tests.rs          — List literals, concatenation, all/exists/exists_one/filter/map macros
//   variable_tests.rs      — input/data variable access, field access (simple, nested, deep)
//   has_tests.rs           — has() macro: basic, nested, data variable, null, in expressions
//   string_tests.rs        — String literals, concatenation, size/startsWith/endsWith/contains/matches
//   in_operator_tests.rs   — `in` operator for lists and maps, complex logical combos
//   numeric_tests.rs       — Uint, cross-type equality/ordering, string/bool/map/list comparisons
//   struct_tests.rs        — Struct literal creation and struct equality
//   container_tests.rs     — Container name resolution (no schema, with container but no schema)
//   extension_tests.rs     — Extension function registration and invocation
//   kubernetes_tests.rs    — Kubernetes list extension tests

use ferricel_core::compiler::{CompilerOptions, compile_cel_to_wasm};
use ferricel_core::runtime::CelEngine;
use ferricel_types::LogLevel;
use slog::{Drain, Logger, o};

// Re-export so test files can reference these types directly after `use common::*;`.
pub(crate) use ferricel_types::extensions::ExtensionDecl;

/// Create a logger suitable for use in tests (writes to stderr).
pub(crate) fn create_test_logger() -> Logger {
    let decorator = slog_term::PlainSyncDecorator::new(std::io::stderr());
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    Logger::root(drain, o!())
}

/// Compile `cel_expr` and execute it, returning the result as `i64`.
///
/// Booleans are mapped to `1` (true) and `0` (false).
pub(crate) fn compile_and_execute(cel_expr: &str) -> Result<i64, anyhow::Error> {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm(cel_expr, compiler_options)?;
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)?;

    // The JSON will be either an integer (e.g. "42") or boolean ("true"/"false").
    let value: serde_json::Value = serde_json::from_str(&json_result)?;

    match value {
        serde_json::Value::Number(n) => n
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Expected i64, got: {}", n)),
        serde_json::Value::Bool(b) => Ok(if b { 1 } else { 0 }),
        _ => anyhow::bail!("Unexpected JSON value type: {}", value),
    }
}

/// Compile `cel_expr` with optional `input` and `data` bindings and execute it,
/// returning the result as `i64`.
///
/// Booleans are mapped to `1` (true) and `0` (false).
pub(crate) fn compile_and_execute_with_vars(
    cel_expr: &str,
    input_json: Option<&str>,
    data_json: Option<&str>,
) -> Result<i64, anyhow::Error> {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm(cel_expr, compiler_options)?;

    let mut bindings = serde_json::Map::new();
    if let Some(val_str) = input_json {
        let val: serde_json::Value = serde_json::from_str(val_str)?;
        bindings.insert("input".to_string(), val);
    }
    if let Some(val_str) = data_json {
        let val: serde_json::Value = serde_json::from_str(val_str)?;
        bindings.insert("data".to_string(), val);
    }
    let bindings_str = serde_json::to_string(&bindings)?;
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, Some(&bindings_str))?;

    let value: serde_json::Value = serde_json::from_str(&json_result)?;

    match value {
        serde_json::Value::Number(n) => n
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Expected i64, got: {}", n)),
        serde_json::Value::Bool(b) => Ok(if b { 1 } else { 0 }),
        _ => anyhow::bail!("Unexpected JSON value type: {}", value),
    }
}

/// Compile `cel_expr` and execute it, returning the result as `f64`.
pub(crate) fn compile_and_execute_double(cel_expr: &str) -> Result<f64, anyhow::Error> {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm(cel_expr, compiler_options)?;
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)?;

    let value: serde_json::Value = serde_json::from_str(&json_result)?;

    match value {
        serde_json::Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| anyhow::anyhow!("Expected f64, got: {}", n)),
        _ => anyhow::bail!("Unexpected JSON value type: {}", value),
    }
}

/// Compile `cel_expr` and execute it, returning the result as a `String`.
pub(crate) fn compile_and_execute_string(cel_expr: &str) -> Result<String, anyhow::Error> {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm(cel_expr, compiler_options)?;
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)?;

    let value: serde_json::Value = serde_json::from_str(&json_result)?;

    match value {
        serde_json::Value::String(s) => Ok(s),
        _ => anyhow::bail!("Expected string, got: {}", value),
    }
}

/// Compile `cel_expr` and execute it, returning the raw JSON result value.
///
/// Useful for tests that need to inspect structured output (e.g. structs/maps).
pub(crate) fn compile_and_execute_json(cel_expr: &str) -> Result<serde_json::Value, anyhow::Error> {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm(cel_expr, compiler_options)?;
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)?;
    Ok(serde_json::from_str(&json_result)?)
}

/// Compile `cel_expr` with an optional container name and proto descriptor,
/// returning the raw WASM bytes (does not execute).
pub(crate) fn compile_with_container(
    cel_expr: &str,
    container: Option<&str>,
    proto_descriptor: Option<Vec<u8>>,
) -> Result<Vec<u8>, anyhow::Error> {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor,
        container: container.map(|s| s.to_string()),
        logger,
        extensions: vec![],
    };
    compile_cel_to_wasm(cel_expr, compiler_options)
}

/// Build a [`CelEngine`] pre-loaded with a single extension function that
/// returns the sum of all integer arguments.
///
/// Returns both the engine (for `execute`) and the [`ExtensionDecl`] (for
/// passing to `compile_cel_to_wasm` via [`CompilerOptions`]).
pub(crate) fn make_engine_with_sum(
    namespace: Option<&str>,
    function: &str,
    num_args: usize,
    receiver_style: bool,
    global_style: bool,
) -> (CelEngine, ExtensionDecl) {
    let logger = create_test_logger();
    let decl = ExtensionDecl {
        namespace: namespace.map(|s| s.to_string()),
        function: function.to_string(),
        receiver_style,
        global_style,
        num_args,
    };
    let mut engine = CelEngine::new(logger);
    engine.register_extension(decl.clone(), |args| {
        let sum: i64 = args.iter().filter_map(|v| v.as_i64()).sum();
        Ok(serde_json::Value::Number(sum.into()))
    });
    (engine, decl)
}

/// Build [`CompilerOptions`] with a single extension declaration and no other
/// customisation.
pub(crate) fn options_with_ext(decl: ExtensionDecl) -> CompilerOptions {
    CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: create_test_logger(),
        extensions: vec![decl],
    }
}
