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
use cel::parser::Parser;
use ferricel_types::functions::RuntimeFunction;
use walrus::{FunctionBuilder, ModuleConfig, ValType};

use crate::schema::ProtoSchema;
use context::{CompilerContext, CompilerEnv};

// Re-export the public API types
pub use context::{CompilerOptions, ExtensionKey};

// Embed the runtime WASM at compile time
const RUNTIME_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../target/wasm32-unknown-unknown/release/runtime.wasm"
));

/// Compile a CEL expression into a WebAssembly module with options
///
/// Takes a CEL expression string and compilation options, returns the compiled WASM module as bytes.
/// The resulting module exports a `validate` function with signature (i32, i32) -> i64.
/// The returned i64 encodes a pointer (low 32 bits) and length (high 32 bits) to
/// JSON-serialized result in WASM memory.
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
    let root_ast = Parser::default()
        .parse(cel_code)
        .map_err(|e| anyhow::anyhow!("Parse error: {:?}", e))?;

    // 5. Build the 'validate' function (i64) -> i64
    // Parameter: encoded (ptr, len) for bindings JSON (map of variable names to values)
    let mut validate_func =
        FunctionBuilder::new(&mut module.types, &[ValType::I64], &[ValType::I64]);
    let bindings_encoded_arg = module.locals.add(ValType::I64);

    let mut body = validate_func.func_body();

    // 6. Initialize global bindings map
    // Deserialize bindings (single parameter) and store in global
    body.local_get(bindings_encoded_arg)
        .call(env.get(RuntimeFunction::DeserializeJson)) // Returns *mut CelValue (should be a Map)
        .call(env.get(RuntimeFunction::InitBindings)); // Store in BINDINGS global

    // 7. Walk the AST and compile to WASM instructions
    // This leaves a *mut CelValue on the stack
    let ctx = CompilerContext::new(
        schema,
        options.container,
        options.logger,
        &options.extensions,
    );
    expr::compile_expr(&root_ast.expr, &mut body, &env, &ctx, &mut module)?;

    // 8. Serialize the result to JSON
    // The stack has a *mut CelValue, serialize it directly
    body.call(env.get(RuntimeFunction::SerializeValue));

    // 9. Finish the function definition
    let validate_id = validate_func.finish(vec![bindings_encoded_arg], &mut module.funcs);

    // 10. Export the 'validate' function for the Host
    module.exports.add("validate", validate_id);

    // 11. Run garbage collection to remove unreferenced items (dead code elimination)
    walrus::passes::gc::run(&mut module);

    // 12. Emit the module as bytes
    Ok(module.emit_wasm())
}
