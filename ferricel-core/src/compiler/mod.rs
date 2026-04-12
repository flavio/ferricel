pub mod access;
pub mod collections;
pub mod context;
pub mod expr;
pub mod functions;
pub mod helpers;
pub mod literals;
pub mod operators;

use std::collections::HashMap;

use anyhow::Context;
use cel::common::ast::Expr;
use cel::parser::Parser;
use ferricel_types::functions::RuntimeFunction;
use walrus::{FunctionBuilder, FunctionId, ModuleConfig, ValType};

use crate::schema::ProtoSchema;
use context::{CompilerContext, CompilerEnv};

// Re-export the public API types
pub use context::{CompilerOptions, ExtensionKey};

// Embed the runtime WASM at compile time.
// The build script (build.rs) copies the WASM into OUT_DIR, resolving it from
// either the workspace target directory (development) or a bundled file
// placed by `make publish-prep` (when publishing to crates.io).
const RUNTIME_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime.wasm"));

/// Compile a CEL expression into a WebAssembly module with options
///
/// Takes a CEL expression string and compilation options, returns the compiled WASM module as bytes.
/// The resulting module exports two functions:
///
/// - `validate(i64) -> i64`: takes JSON-encoded bindings, returns JSON-encoded result
/// - `validate_proto(i64) -> i64`: takes protobuf-encoded `ferricel.Bindings`, returns JSON-encoded result
///
/// The i64 encoding packs ptr (low 32 bits) and len (high 32 bits) into a single value.
///
/// # Arguments
///
/// * `cel_code` - The CEL expression to compile
/// * `options` - Compilation options including optional proto schema
///
/// # Example
///
/// ```no_run
/// use ferricel_core::{compile_cel_to_wasm, CompilerOptions, ProtoSchema};
///
/// let descriptor_bytes = std::fs::read("types.pb").unwrap();
/// let options = CompilerOptions {
///     proto_descriptor: Some(descriptor_bytes),
///     container: None,
///     logger: slog::Logger::root(slog::Discard, slog::o!()),
///     extensions: vec![],
/// };
/// let wasm_bytes = compile_cel_to_wasm("TestAllTypes{}.field", options).unwrap();
/// ```
pub fn compile_cel_to_wasm(
    cel_code: &str,
    options: CompilerOptions,
) -> Result<Vec<u8>, anyhow::Error> {
    // 1. Parse proto schema if provided
    let schema = options
        .proto_descriptor
        .as_ref()
        .map(|bytes| ProtoSchema::from_descriptor_set(bytes))
        .transpose()?;

    // 2. Load the runtime template from embedded bytes
    let mut module = ModuleConfig::new().parse(RUNTIME_BYTES)?;

    // 3. Set up the compiler environment and manage exports
    let mut functions = HashMap::new();

    for func in RuntimeFunction::iter() {
        let id = module.exports.get_func(func.name()).with_context(|| {
            format!(
                "Runtime function '{}' not found in module exports",
                func.name()
            )
        })?;

        functions.insert(func, id);

        // If it shouldn't be exported, remove it
        if !func.is_exported() {
            module.exports.remove(func.name())?;
        }
    }

    let env = CompilerEnv { functions };

    // 4. Parse the CEL expression
    let root_ast = Parser::new()
        .enable_optional_syntax(true)
        .parse(cel_code)
        .map_err(|e| anyhow::anyhow!("Parse error: {:?}", e))?;

    let ctx = CompilerContext::new(
        schema,
        options.container,
        options.logger,
        &options.extensions,
    );

    // 5. Build the 'validate' function (i64) -> i64 — JSON bindings path
    let validate_id = build_validate_function(&mut module, &env, &ctx, &root_ast.expr)?;
    module.exports.add("validate", validate_id);

    // 6. Build the 'validate_proto' function (i64) -> i64 — protobuf bindings path
    let validate_proto_id = build_validate_proto_function(&mut module, &env, &ctx, &root_ast.expr)?;
    module.exports.add("validate_proto", validate_proto_id);

    // 7. Run garbage collection to remove unreferenced items (dead code elimination)
    walrus::passes::gc::run(&mut module);

    // 8. Emit the module as bytes
    Ok(module.emit_wasm())
}

/// Build the `validate` WASM function `(i64) -> i64` using JSON-encoded bindings.
///
/// Deserializes bindings with [`RuntimeFunction::DeserializeJson`], evaluates the expression,
/// and serializes the result back to JSON.
fn build_validate_function(
    module: &mut walrus::Module,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    expr: &Expr,
) -> Result<FunctionId, anyhow::Error> {
    let mut func = FunctionBuilder::new(&mut module.types, &[ValType::I64], &[ValType::I64]);
    let bindings_encoded_arg = module.locals.add(ValType::I64);
    let mut body = func.func_body();

    body.local_get(bindings_encoded_arg)
        .call(env.get(RuntimeFunction::DeserializeJson))
        .call(env.get(RuntimeFunction::InitBindings));

    expr::compile_expr(expr, &mut body, env, ctx, module)?;

    body.call(env.get(RuntimeFunction::SerializeValue));

    Ok(func.finish(vec![bindings_encoded_arg], &mut module.funcs))
}

/// Build the `validate_proto` WASM function `(i64) -> i64` using protobuf-encoded bindings.
///
/// Deserializes bindings with [`RuntimeFunction::DeserializeProto`], evaluates the expression,
/// and serializes the result back to JSON.
fn build_validate_proto_function(
    module: &mut walrus::Module,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    expr: &Expr,
) -> Result<FunctionId, anyhow::Error> {
    let mut func = FunctionBuilder::new(&mut module.types, &[ValType::I64], &[ValType::I64]);
    let bindings_encoded_arg = module.locals.add(ValType::I64);
    let mut body = func.func_body();

    body.local_get(bindings_encoded_arg)
        .call(env.get(RuntimeFunction::DeserializeProto))
        .call(env.get(RuntimeFunction::InitBindings));

    expr::compile_expr(expr, &mut body, env, ctx, module)?;

    body.call(env.get(RuntimeFunction::SerializeValue));

    Ok(func.finish(vec![bindings_encoded_arg], &mut module.funcs))
}
