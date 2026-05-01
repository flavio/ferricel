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
use ferricel_types::{extensions::ExtensionDecl, functions::RuntimeFunction};
use walrus::{FunctionBuilder, FunctionId, ModuleConfig, ValType};

use context::{CompilerContext, CompilerEnv};

// Re-export the public API types
pub use context::ExtensionKey;

// Embed the runtime WASM at compile time.
// The build script (build.rs) copies the WASM into OUT_DIR, resolving it from
// either the workspace target directory (development) or a bundled file
// placed by `make publish-prep` (when publishing to crates.io).
const RUNTIME_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime.wasm"));

use crate::schema::ProtoSchema;

/// Builder for configuring and constructing a [`Compiler`].
///
/// All builder methods are consuming (take and return `Self`).
/// Call [`compiler::Builder::build`](Builder::build) to obtain an immutable [`Compiler`].
///
/// # Example
///
/// ```no_run
/// use ferricel_core::compiler::Builder;
///
/// let compiler = Builder::new()
///     .with_container("my.namespace")
///     .build();
///
/// let wasm_bytes = compiler.compile("1 + 1").unwrap();
/// ```
pub struct Builder {
    proto_descriptor: Option<Vec<u8>>,
    container: Option<String>,
    logger: slog::Logger,
    extensions: Vec<ExtensionDecl>,
}

impl Builder {
    /// Create a new builder with sensible defaults.
    ///
    /// The default logger discards all output.  Override it with
    /// [`with_logger`](Self::with_logger) if you need log output.
    pub fn new() -> Self {
        Self {
            proto_descriptor: None,
            container: None,
            logger: slog::Logger::root(slog::Discard, slog::o!()),
            extensions: vec![],
        }
    }

    /// Override the logger used during compilation.
    pub fn with_logger(mut self, logger: slog::Logger) -> Self {
        self.logger = logger;
        self
    }

    /// Set a Protocol Buffer descriptor set (binary `FileDescriptorSet`).
    ///
    /// The bytes are stored and parsed eagerly when this method is called,
    /// so bad input is rejected immediately.
    pub fn with_proto_descriptor(mut self, bytes: Vec<u8>) -> Result<Self, anyhow::Error> {
        // Validate eagerly by attempting to parse.
        ProtoSchema::from_descriptor_set(&bytes)?;
        self.proto_descriptor = Some(bytes);
        Ok(self)
    }

    /// Set the container (namespace) used for CEL type-name resolution.
    pub fn with_container(mut self, container: impl Into<String>) -> Self {
        self.container = Some(container.into());
        self
    }

    /// Append one extension function declaration.
    ///
    /// May be called multiple times to register several extensions.
    pub fn with_extension(mut self, decl: ExtensionDecl) -> Self {
        self.extensions.push(decl);
        self
    }

    /// Consume the builder and produce an immutable [`Compiler`].
    ///
    /// This is infallible — all fallible work (descriptor parsing) is done in
    /// [`with_proto_descriptor`](Self::with_proto_descriptor).
    pub fn build(self) -> Compiler {
        // Parse the schema; we already validated it above, so unwrap is safe.
        let schema = self.proto_descriptor.as_ref().map(|b| {
            ProtoSchema::from_descriptor_set(b).expect("proto descriptor already validated")
        });

        Compiler {
            schema,
            container: self.container,
            logger: self.logger,
            extensions: self.extensions,
        }
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// An immutable CEL-to-WASM compiler.
///
/// Construct via [`Builder`].  The parsed `ProtoSchema` (if any) is
/// ready at construction time and reused across every call to [`compile`](Self::compile).
pub struct Compiler {
    schema: Option<ProtoSchema>,
    container: Option<String>,
    logger: slog::Logger,
    extensions: Vec<ExtensionDecl>,
}

impl Compiler {
    /// Compile a CEL expression into a WebAssembly module.
    ///
    /// Returns the compiled WASM module as bytes.
    /// The resulting module exports two functions:
    ///
    /// - `evaluate(i64) -> i64`:       takes JSON-encoded bindings, returns JSON-encoded result
    /// - `evaluate_proto(i64) -> i64`: takes protobuf-encoded `ferricel.Bindings`, returns JSON-encoded result
    ///
    /// Both functions return a packed ptr+len i64 on success.  If the CEL expression
    /// produces a runtime error (overflow, divide-by-zero, etc.) the WASM traps via
    /// `cel_abort`, and the host receives `Err(...)` from the call.
    ///
    /// The i64 packs ptr (low 32 bits) and len (high 32 bits) into a single value.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ferricel_core::compiler::CompilerBuilder;
    ///
    /// let compiler = CompilerBuilder::new().build();
    /// let wasm_bytes = compiler.compile("1 + 1").unwrap();
    /// ```
    pub fn compile(&self, cel_code: &str) -> Result<Vec<u8>, anyhow::Error> {
        // 1. Load the runtime template from embedded bytes
        let mut module = ModuleConfig::new().parse(RUNTIME_BYTES)?;

        // 2. Set up the compiler environment and manage exports
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

        // 3. Parse the CEL expression
        let root_ast = Parser::new()
            .enable_optional_syntax(true)
            .parse(cel_code)
            .map_err(|e| anyhow::anyhow!("Parse error: {:?}", e))?;

        let ctx = CompilerContext::new(
            self.schema.clone(),
            self.container.clone(),
            self.logger.clone(),
            &self.extensions,
        );

        // 4. Build the 'evaluate' function (i64) -> (i32, i64) — JSON bindings path
        let evaluate_id = build_evaluate_function(&mut module, &env, &ctx, &root_ast.expr)?;
        module.exports.add("evaluate", evaluate_id);

        // 5. Build the 'evaluate_proto' function (i64) -> (i32, i64) — protobuf bindings path
        let evaluate_proto_id =
            build_evaluate_proto_function(&mut module, &env, &ctx, &root_ast.expr)?;
        module.exports.add("evaluate_proto", evaluate_proto_id);

        // 6. Run garbage collection to remove unreferenced items (dead code elimination)
        walrus::passes::gc::run(&mut module);

        // 7. Emit the module as bytes
        Ok(module.emit_wasm())
    }
}

/// Build the `evaluate` WASM function `(i64) -> i64` using JSON-encoded bindings.
///
/// Deserializes bindings with [`RuntimeFunction::DeserializeJson`], evaluates the expression,
/// and serializes the result via [`RuntimeFunction::SerializeResult`].  If the result is a
/// `CelValue::Error`, `SerializeResult` calls `cel_abort` which traps the WASM instance,
/// propagating the error as `Err(...)` on the host side.
fn build_evaluate_function(
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

    body.call(env.get(RuntimeFunction::SerializeResult));

    Ok(func.finish(vec![bindings_encoded_arg], &mut module.funcs))
}

/// Build the `evaluate_proto` WASM function `(i64) -> i64` using protobuf-encoded bindings.
///
/// Deserializes bindings with [`RuntimeFunction::DeserializeProto`], evaluates the expression,
/// and serializes the result via [`RuntimeFunction::SerializeResult`].  If the result is a
/// `CelValue::Error`, `SerializeResult` calls `cel_abort` which traps the WASM instance,
/// propagating the error as `Err(...)` on the host side.
fn build_evaluate_proto_function(
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

    body.call(env.get(RuntimeFunction::SerializeResult));

    Ok(func.finish(vec![bindings_encoded_arg], &mut module.funcs))
}
