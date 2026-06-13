pub mod access;
pub mod collections;
pub mod context;
pub mod expr;
pub mod functions;
pub mod helpers;
pub mod literals;
pub mod operators;
#[cfg(feature = "k8s-vap")]
#[cfg_attr(docsrs, doc(cfg(feature = "k8s-vap")))]
pub mod vap;

use std::collections::{BTreeSet, HashMap};

use anyhow::Context;
use cel::{common::ast::Expr, parser::Parser};
// Re-export the public API types
pub use context::ExtensionKey;
use context::{CompilerContext, CompilerEnv};
use ferricel_types::{
    extensions::{BuilderChainDecl, ExtensionDecl},
    functions::RuntimeFunction,
};
use walrus::{FunctionBuilder, FunctionId, ModuleConfig, ValType};

// Embed the runtime Wasm at compile time.
// The build script (build.rs) copies the Wasm into OUT_DIR, resolving it from
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
    extensions: BTreeSet<ExtensionDecl>,
    builder_chains: Vec<BuilderChainDecl>,
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
            extensions: BTreeSet::new(),
            builder_chains: Vec::new(),
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
        self.extensions.insert(decl);
        self
    }

    /// Register a fluent builder chain extension family (e.g. `kw.k8s`, `kw.sigstore`).
    ///
    /// May be called multiple times to register several chains.
    pub fn with_builder_chain(mut self, decl: BuilderChainDecl) -> Self {
        self.builder_chains.push(decl);
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
            builder_chains: self.builder_chains,
        }
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// An immutable CEL-to-Wasm compiler.
///
/// Construct via [`Builder`].  The parsed `ProtoSchema` (if any) is
/// ready at construction time and reused across every call to [`compile`](Self::compile).
pub struct Compiler {
    schema: Option<ProtoSchema>,
    container: Option<String>,
    logger: slog::Logger,
    extensions: BTreeSet<ExtensionDecl>,
    builder_chains: Vec<BuilderChainDecl>,
}

impl Compiler {
    /// Compile a CEL expression into a WebAssembly module.
    ///
    /// Returns the compiled Wasm module as bytes.
    /// The resulting module exports two functions:
    ///
    /// - `evaluate(i64) -> i64`:       takes JSON-encoded bindings, returns JSON-encoded result
    /// - `evaluate_proto(i64) -> i64`: takes protobuf-encoded `ferricel.Bindings`, returns JSON-encoded result
    ///
    /// Both functions return a packed ptr+len i64 on success.  If the CEL expression
    /// produces a runtime error (overflow, divide-by-zero, etc.) the Wasm traps via
    /// `cel_abort`, and the host receives `Err(...)` from the call.
    ///
    /// The i64 packs ptr (low 32 bits) and len (high 32 bits) into a single value.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ferricel_core::compiler::Builder;
    ///
    /// let compiler = Builder::new().build();
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
            &self.builder_chains,
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

        // 7. Populate the producers custom section
        add_producers_entries(&mut module);

        // 8. Emit the module as bytes
        Ok(module.emit_wasm())
    }

    /// Compile a Kubernetes `ValidatingAdmissionPolicy` from its YAML manifest
    /// into a self-contained WebAssembly module.
    ///
    /// The resulting module exports:
    ///
    /// - `evaluate(i64) -> i64` — JSON-encoded bindings input
    ///
    /// The result JSON is a [Kubewarden](https://kubewarden.io/)
    /// [`ValidationResponse`](https://docs.kubewarden.io/admission-controller/1.35/en/reference/spec/03-validating-policies.html#_the_validationresponse_object):
    ///
    /// Accepted response:
    /// ```json
    /// {"accepted": true}
    /// ```
    ///
    /// Rejected response:
    /// ```json
    /// {"accepted": false, "message": "...", "code": 422}
    /// ```
    ///
    /// The YAML must contain exactly one `ValidatingAdmissionPolicy` document.
    /// The caller must pass, at minimum, `object` in the bindings. When the
    /// policy references `namespaceObject` or `params`, the host must also
    /// register `kubernetes.get` / `kubernetes.list` extension implementations
    /// on the `Engine`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ferricel_core::compiler::Builder;
    ///
    /// let yaml = std::fs::read_to_string("policy.yaml").unwrap();
    /// let compiler = Builder::new().build();
    /// let wasm_bytes = compiler.compile_vap(&yaml).unwrap();
    /// ```
    #[cfg(feature = "k8s-vap")]
    #[cfg_attr(docsrs, doc(cfg(feature = "k8s-vap")))]
    pub fn compile_vap(&self, vap_yaml: &str) -> Result<Vec<u8>, anyhow::Error> {
        let spec = parse_vap_yaml(vap_yaml)?;
        self.compile_vap_from_spec(&spec)
    }

    /// Compile a `ValidatingAdmissionPolicySpec` directly (bypassing YAML parsing).
    ///
    /// Useful when the caller has already parsed or constructed the spec
    /// programmatically from a `k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicySpec`.
    #[cfg(feature = "k8s-vap")]
    #[cfg_attr(docsrs, doc(cfg(feature = "k8s-vap")))]
    pub fn compile_vap_from_spec(
        &self,
        spec: &k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicySpec,
    ) -> Result<Vec<u8>, anyhow::Error> {
        let extensions = self.extensions.clone();

        // Merge user-supplied builder chains with the implicit kw.k8s chain.
        let mut builder_chains = self.builder_chains.clone();
        builder_chains.push(vap::kw_k8s_chain());

        // Load runtime template
        let mut module = ModuleConfig::new().parse(RUNTIME_BYTES)?;

        // Build CompilerEnv
        let mut functions = HashMap::new();
        for func in RuntimeFunction::iter() {
            let id = module.exports.get_func(func.name()).with_context(|| {
                format!(
                    "Runtime function '{}' not found in module exports",
                    func.name()
                )
            })?;
            functions.insert(func, id);
            if !func.is_exported() {
                module.exports.remove(func.name())?;
            }
        }
        let env = CompilerEnv { functions };

        let ctx = CompilerContext::new(
            self.schema.clone(),
            self.container.clone(),
            self.logger.clone(),
            &extensions,
            &builder_chains,
        );

        // Build `evaluate` (JSON bindings)
        let evaluate_id = vap::build_vap_evaluate_function(&mut module, &env, &ctx, spec)?;
        module.exports.add("evaluate", evaluate_id);

        walrus::passes::gc::run(&mut module);
        add_producers_entries(&mut module);
        Ok(module.emit_wasm())
    }
}

/// Populate the standardised WebAssembly `producers` section.
///
/// Entries are **added to** (not replaced) whatever the embedded `runtime.wasm`
/// template already recorded (typically `rustc` / `LLVM` entries from the Rust
/// toolchain).  This follows the tool-conventions spec, which explicitly states
/// that "it is possible (and common) for multiple tools to be used in the
/// overall pipeline that produces and optimizes a given wasm module".
///
/// Adds:
/// - `language`:     `"CEL"` (empty version — CEL has no distinct release cycle
///   separate from this compiler)
/// - `processed-by`: `"ferricel"` with the crate version from `CARGO_PKG_VERSION`
fn add_producers_entries(module: &mut walrus::Module) {
    module.producers.add_language("CEL", "");
    module
        .producers
        .add_processed_by("ferricel", env!("CARGO_PKG_VERSION"));
}

/// Build the `evaluate` Wasm function `(i64) -> i64` using JSON-encoded bindings.
///
/// Deserializes bindings with [`RuntimeFunction::DeserializeJson`], evaluates the expression,
/// and serializes the result via [`RuntimeFunction::SerializeResult`].  If the result is a
/// `CelValue::Error`, `SerializeResult` calls `cel_abort` which traps the Wasm instance,
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

/// Build the `evaluate_proto` Wasm function `(i64) -> i64` using protobuf-encoded bindings.
///
/// Deserializes bindings with [`RuntimeFunction::DeserializeProto`], evaluates the expression,
/// and serializes the result via [`RuntimeFunction::SerializeResult`].  If the result is a
/// `CelValue::Error`, `SerializeResult` calls `cel_abort` which traps the Wasm instance,
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

// ─── VAP YAML parsing ─────────────────────────────────────────────────────────

/// Parse a `ValidatingAdmissionPolicy` YAML string into a
/// `ValidatingAdmissionPolicySpec`.
///
/// The YAML must contain exactly one `ValidatingAdmissionPolicy` document; an
/// error is returned if zero or more than one document is found.
#[cfg(feature = "k8s-vap")]
pub(crate) fn parse_vap_yaml(
    yaml: &str,
) -> Result<k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicySpec, anyhow::Error>
{
    use k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicy;
    use serde::Deserialize as _;

    let mut iter = yaml_serde::Deserializer::from_str(yaml);

    // Deserialize the first document.
    let first = iter
        .next()
        .ok_or_else(|| anyhow::anyhow!("YAML is empty, expected a ValidatingAdmissionPolicy"))?;
    let policy = ValidatingAdmissionPolicy::deserialize(first)
        .map_err(|e| anyhow::anyhow!("Failed to parse ValidatingAdmissionPolicy YAML: {}", e))?;

    // Reject multi-document YAML files.
    if iter.next().is_some() {
        anyhow::bail!(
            "Expected exactly one ValidatingAdmissionPolicy document, but found more than one"
        );
    }

    let spec = policy
        .spec
        .ok_or_else(|| anyhow::anyhow!("ValidatingAdmissionPolicy has no spec"))?;

    if spec.validations.as_deref().unwrap_or(&[]).is_empty() {
        anyhow::bail!("ValidatingAdmissionPolicy must have at least one validation");
    }

    Ok(spec)
}
