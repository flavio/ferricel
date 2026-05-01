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
//   namespace_tests.rs     — Container/namespace name resolution (qualified vars, container prefixes, comprehension shadowing)
//   extension_tests.rs     — Extension function registration and invocation
//   kubernetes_tests.rs    — Kubernetes list extension tests

use ferricel_core::compiler::Builder;
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

/// Compile `cel_expr` and execute it with no bindings, returning the result as
/// a [`serde_json::Value`].
///
/// This is the base helper.  Use the typed wrappers below when a specific Rust
/// type is more convenient.
pub(crate) fn compile_and_execute(cel_expr: &str) -> Result<serde_json::Value, anyhow::Error> {
    compile_and_execute_with_vars(cel_expr, None)
}

/// Compile `cel_expr` and execute it with an optional JSON bindings object,
/// returning the result as a [`serde_json::Value`].
///
/// The bindings string should be a JSON object whose keys are variable names,
/// e.g. `r#"{"input": 42}"#`.
pub(crate) fn compile_and_execute_with_vars(
    cel_expr: &str,
    bindings: Option<&str>,
) -> Result<serde_json::Value, anyhow::Error> {
    let logger = create_test_logger();
    let wasm_bytes = Builder::new()
        .with_logger(logger.clone())
        .build()
        .compile(cel_expr)?;
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, bindings)?;
    Ok(serde_json::from_str(&json_result)?)
}

/// Compile `cel_expr` with separate optional `input` and `data` JSON bindings
/// and execute it, returning the result as a [`serde_json::Value`].
///
/// This is a convenience wrapper for the common test pattern where bindings are
/// provided as two separate top-level variables.
pub(crate) fn compile_and_execute_with_input_data(
    cel_expr: &str,
    input_json: Option<&str>,
    data_json: Option<&str>,
) -> Result<serde_json::Value, anyhow::Error> {
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
    compile_and_execute_with_vars(cel_expr, Some(&bindings_str))
}

/// Compile `cel_expr` and execute it, returning the result as `f64`.
pub(crate) fn compile_and_execute_bool(cel_expr: &str) -> Result<bool, anyhow::Error> {
    let value = compile_and_execute(cel_expr)?;
    value
        .as_bool()
        .ok_or_else(|| anyhow::anyhow!("expected bool, got: {}", value))
}

pub(crate) fn compile_and_execute_double(cel_expr: &str) -> Result<f64, anyhow::Error> {
    let value = compile_and_execute(cel_expr)?;
    value
        .as_f64()
        .ok_or_else(|| anyhow::anyhow!("expected f64, got: {}", value))
}

/// Compile `cel_expr` and execute it, returning the result as a `String`.
pub(crate) fn compile_and_execute_string(cel_expr: &str) -> Result<String, anyhow::Error> {
    let value = compile_and_execute(cel_expr)?;
    match value {
        serde_json::Value::String(s) => Ok(s),
        other => anyhow::bail!("expected string, got: {}", other),
    }
}

/// Compile `cel_expr` with an optional container name and proto descriptor,
/// returning the raw WASM bytes (does not execute).
pub(crate) fn compile_with_container(
    cel_expr: &str,
    container: Option<&str>,
    proto_descriptor: Option<Vec<u8>>,
) -> Result<Vec<u8>, anyhow::Error> {
    let mut builder = Builder::new().with_logger(create_test_logger());
    if let Some(bytes) = proto_descriptor {
        builder = builder.with_proto_descriptor(bytes)?;
    }
    if let Some(c) = container {
        builder = builder.with_container(c);
    }
    builder.build().compile(cel_expr)
}

/// Build a [`CelEngine`] pre-loaded with a single extension function that
/// returns the sum of all integer arguments.
///
/// Returns both the engine (for `execute`) and the [`ExtensionDecl`] (for
/// passing to [`compiler::Builder`](ferricel_core::compiler::Builder) via
/// [`with_extension`](ferricel_core::compiler::Builder::with_extension)).
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

/// Build a [`Compiler`] with a single extension declaration and no other
/// customisation.
pub(crate) fn options_with_ext(decl: ExtensionDecl) -> ferricel_core::compiler::Compiler {
    Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl)
        .build()
}
