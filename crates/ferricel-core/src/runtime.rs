//! Wasm runtime for compiled CEL modules.
//!
//! Use [`Builder`] to configure and execute Wasm modules produced by the
//! [compiler](crate::compiler). Register host-provided extension functions
//! with [`Builder::with_extension`].
//!
//! See the [Host Extensions](https://flavio.github.io/ferricel/host-extensions.html)
//! chapter of the user guide for details on flat extensions and builder chains.
//!
//! # Runtime errors
//!
//! When the CEL expression produces a runtime error (divide by zero, an
//! unbound variable, a failed extension call, and so on), [`Engine::eval`]
//! and [`Engine::eval_proto`] return an `anyhow::Error` that downcasts to
//! [`CelRuntimeError`]:
//!
//! ```ignore
//! match engine.eval(Some(bindings)) {
//!     Ok(result) => { /* JSON-encoded CEL value */ }
//!     Err(err) => match err.downcast_ref::<CelRuntimeError>() {
//!         Some(cel_err) => { /* the CEL expression evaluated to an error */ }
//!         None => { /* deadline, memory limit, trap, host bug, ... */ }
//!     },
//! }
//! ```
//!
//! Other failures (an epoch-deadline interrupt, a memory-limit abort, a Wasm
//! trap, a missing export) do not downcast to [`CelRuntimeError`].
//!
//! # ABI version
//!
//! [`Builder::build_pre`] checks the module's `ferricel.abi-version` custom
//! section against [`ferricel_types::ABI_VERSION`] and returns `Err` on a
//! mismatch or a missing section, before it links or instantiates the
//! module. This check runs only for [`Builder::with_wasm`]; see
//! [`Builder::with_module`] for the pre-compiled path.

pub use ferricel_types::{CelRuntimeError, ExtensionOrigin};
use ferricel_types::{
    LogLevel,
    extensions::{ExtensionCallResponse, ExtensionDecl},
};
use serde::Deserialize;
use wasmtime::{Caller, Engine as WasmEngine, InstancePre, Linker, Module, Store};

use crate::compiler::ExtensionKey;

/// Type alias for an extension function implementation.
///
/// The runtime makes sure that `args.len()` equals [`ExtensionDecl::num_args`]
/// before it calls this function. A call with the wrong count is rejected
/// first (see [`Extensions`]). An implementation does not have to check the
/// argument count again.
pub type ExtensionFn = std::sync::Arc<
    dyn Fn(Vec<serde_json::Value>) -> Result<serde_json::Value, String> + Send + Sync,
>;

/// A registered host extension: the declaration and the implementation.
#[derive(Clone)]
pub struct Extension {
    /// The declaration. The compiler uses it to type-check call sites. The
    /// runtime uses it to check the argument count.
    pub decl: ExtensionDecl,
    /// The host implementation.
    pub implementation: ExtensionFn,
}

/// A hook that authorizes an extension call before the runtime parses its
/// arguments.
///
/// The runtime calls this hook with the `(namespace, function)` of the
/// call, before it parses the arguments and before it calls the
/// implementation. `Err(msg)` rejects the call. The guest receives `msg`
/// as a CEL runtime error, the same way it receives an error for an
/// unknown extension or a wrong argument count. Use this hook to reject a
/// call that the current context does not allow, before the runtime does
/// the work of parsing the arguments. Set the hook with
/// [`Extensions::with_extension_authorizer`],
/// [`Extensions::set_extension_authorizer`], or
/// [`Builder::with_extension_authorizer`].
pub type ExtensionAuthorizer =
    std::sync::Arc<dyn Fn(&ExtensionKey) -> Result<(), String> + Send + Sync>;

/// The set of host extension functions that a Wasm module can call during
/// evaluation.
///
/// `ferricel-core` treats every Wasm module as untrusted input. The module
/// can come from a source other than the ferricel compiler. As a result, the
/// arguments in a `cel_call_extension` request can have any count and any
/// size.
///
/// `Extensions` stores each implementation with its [`ExtensionDecl`], and
/// an optional [`ExtensionAuthorizer`]. For each request, the runtime does
/// this work in this order:
///
/// 1. It reads `namespace` and `function` and looks up the extension. An
///    unknown extension is a CEL evaluation error.
/// 2. It calls the extension authorizer, if one is set, with the
///    [`ExtensionKey`]. `Err(msg)` is a CEL evaluation error. The runtime
///    has not parsed `args` at this point.
/// 3. It parses `args`.
/// 4. It makes sure that `args.len() == decl.num_args`. A wrong count (for
///    example, an empty list) is a CEL evaluation error. The runtime never
///    calls the closure with the wrong count.
/// 5. It calls the implementation.
///
/// Build the set with [`Extensions::new`] and [`Extensions::register`] or
/// [`Extensions::with`]. Then pass it to [`EnginePre::rehydrate`].
/// [`Builder::with_extension`] builds the set for you.
#[derive(Clone, Default)]
pub struct Extensions {
    inner: std::collections::HashMap<ExtensionKey, Extension>,
    authorizer: Option<ExtensionAuthorizer>,
}

impl Extensions {
    /// Create an empty extension set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the extension authorizer and return `self`.
    ///
    /// See [`ExtensionAuthorizer`]. A later call replaces it.
    pub fn with_extension_authorizer(
        mut self,
        authorizer: impl Fn(&ExtensionKey) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        self.set_extension_authorizer(authorizer);
        self
    }

    /// Set the extension authorizer in place.
    ///
    /// See [`ExtensionAuthorizer`]. A later call replaces it.
    pub fn set_extension_authorizer(
        &mut self,
        authorizer: impl Fn(&ExtensionKey) -> Result<(), String> + Send + Sync + 'static,
    ) {
        self.authorizer = Some(std::sync::Arc::new(authorizer));
    }

    /// Register an extension implementation and return `self`.
    ///
    /// If an extension with the same `(namespace, function)` exists, this
    /// method replaces it.
    pub fn with(
        mut self,
        decl: ExtensionDecl,
        implementation: impl Fn(Vec<serde_json::Value>) -> Result<serde_json::Value, String>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.register(decl, implementation);
        self
    }

    /// Register an extension implementation in place.
    ///
    /// If an extension with the same `(namespace, function)` exists, this
    /// method replaces it.
    pub fn register(
        &mut self,
        decl: ExtensionDecl,
        implementation: impl Fn(Vec<serde_json::Value>) -> Result<serde_json::Value, String>
        + Send
        + Sync
        + 'static,
    ) {
        self.insert(decl, std::sync::Arc::new(implementation));
    }

    /// Register an existing [`ExtensionFn`] in place.
    pub fn insert(&mut self, decl: ExtensionDecl, implementation: ExtensionFn) {
        let key = ExtensionKey::new(decl.namespace.clone(), decl.function.clone());
        self.inner.insert(
            key,
            Extension {
                decl,
                implementation,
            },
        );
    }

    /// Look up a registered extension by key.
    pub fn get(&self, key: &ExtensionKey) -> Option<&Extension> {
        self.inner.get(key)
    }

    /// Iterate over the declarations of all registered extensions.
    ///
    /// Pass these declarations to [`crate::compiler::Builder::with_extension`].
    /// Then the compiler and the runtime use the same argument count for
    /// each extension.
    pub fn decls(&self) -> impl Iterator<Item = &ExtensionDecl> {
        self.inner.values().map(|ext| &ext.decl)
    }

    /// The number of registered extensions.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether no extensions are registered.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Phase-1 view of an [`ExtensionCallPayload`]: `args` is kept as raw JSON
/// text.
///
/// `ferricel_types::extensions::ExtensionCallPayload` is the wire format
/// that the guest serializes. The host does not deserialize into it.
/// Instead, the host uses this struct, so that it can look up the
/// extension and run the [`ExtensionAuthorizer`] before it builds a
/// `serde_json::Value` tree for `args`. `RawValue` makes serde scan `args`
/// once, to validate its syntax and find its end, without allocating a
/// tree.
///
/// [`ExtensionCallPayload`]: ferricel_types::extensions::ExtensionCallPayload
#[derive(Deserialize)]
struct ExtensionCallEnvelope<'a> {
    namespace: Option<String>,
    function: String,
    #[serde(borrow)]
    args: &'a serde_json::value::RawValue,
}

/// The reason [`dispatch_extension`] failed.
#[derive(Debug, PartialEq)]
enum DispatchError {
    /// The request bytes are not a valid `ExtensionCallPayload`. This is a
    /// bug in the guest, not a CEL error. The host import turns it into a
    /// trap.
    Malformed(String),
    /// The runtime rejected the call (unknown extension, authorizer
    /// denial, or wrong arity), or the implementation returned `Err`. The
    /// guest receives it as [`ExtensionCallResponse::Error`] and reports a
    /// CEL runtime error.
    Runtime(String),
}

/// Dispatch an extension call from the guest to a registered extension.
///
/// `req` holds the JSON `ExtensionCallPayload` that the guest sent. See
/// [`Extensions`] for the reason behind each step. The function works in
/// this order:
///
/// 1. It parses the envelope (`namespace` and `function`), and keeps
///    `args` as raw text. A syntax error becomes
///    [`DispatchError::Malformed`].
/// 2. It looks up the extension. An unknown key becomes
///    [`DispatchError::Runtime`].
/// 3. It calls the [`ExtensionAuthorizer`], if one is set. `Err(msg)`
///    becomes [`DispatchError::Runtime`].
/// 4. It parses `args`. A value that is not a JSON array becomes
///    [`DispatchError::Malformed`]. A single-phase parse into
///    `ExtensionCallPayload` gives the same error for this input.
/// 5. It makes sure that `args.len() == decl.num_args`. A mismatch
///    becomes [`DispatchError::Runtime`].
/// 6. It calls the implementation with `args` moved in. `Err(msg)`
///    becomes [`DispatchError::Runtime`].
fn dispatch_extension(
    extensions: &Extensions,
    req: &[u8],
) -> Result<serde_json::Value, DispatchError> {
    let envelope: ExtensionCallEnvelope<'_> =
        serde_json::from_slice(req).map_err(|e| DispatchError::Malformed(e.to_string()))?;

    let key = ExtensionKey::new(envelope.namespace, envelope.function);
    let full_name = || match &key.namespace {
        Some(ns) => format!("{}.{}", ns, key.function),
        None => key.function.clone(),
    };

    let Some(ext) = extensions.get(&key) else {
        return Err(DispatchError::Runtime(format!(
            "Extension not found: {}",
            full_name()
        )));
    };

    if let Some(authorizer) = &extensions.authorizer {
        authorizer(&key).map_err(DispatchError::Runtime)?;
    }

    let args: Vec<serde_json::Value> = serde_json::from_str(envelope.args.get())
        .map_err(|e| DispatchError::Malformed(format!("invalid `args`: {}", e)))?;

    if args.len() != ext.decl.num_args {
        return Err(DispatchError::Runtime(format!(
            "{} expects {} argument(s), got {}",
            full_name(),
            ext.decl.num_args,
            args.len()
        )));
    }

    (ext.implementation)(args).map_err(DispatchError::Runtime)
}

/// Decode the bytes that the guest passes to `cel_abort`.
///
/// The guest sends a JSON-encoded [`CelRuntimeError`]. Any other content is
/// a bug in the guest, not a CEL runtime error. It produces a generic error
/// that does not downcast to [`CelRuntimeError`].
fn parse_abort_payload(bytes: &[u8]) -> Result<CelRuntimeError, wasmtime::Error> {
    serde_json::from_slice::<CelRuntimeError>(bytes).map_err(|e| {
        wasmtime::Error::msg(format!("Invalid cel_abort payload from the guest: {}", e))
    })
}

/// Copy `len` bytes at `ptr` out of guest memory.
///
/// Both values come from the guest. The function checks them against the
/// memory size before it allocates. As a result, a guest cannot make the
/// host allocate more than the memory holds. The function also skips the
/// zero-fill step that `vec![0u8; len]` needs before `Memory::read`.
///
/// An out-of-bounds range is a bug in the guest. It produces a
/// `wasmtime::Error`, which traps the instance.
fn read_guest_bytes(
    memory: &wasmtime::Memory,
    ctx: impl wasmtime::AsContext,
    ptr: usize,
    len: usize,
) -> Result<Vec<u8>, wasmtime::Error> {
    fn out_of_bounds(ptr: usize, len: usize) -> wasmtime::Error {
        wasmtime::Error::msg(format!("guest pointer out of bounds: ptr={ptr} len={len}"))
    }

    let end = ptr
        .checked_add(len)
        .ok_or_else(|| out_of_bounds(ptr, len))?;
    memory
        .data(&ctx)
        .get(ptr..end)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| out_of_bounds(ptr, len))
}

/// Make sure that `wasm_bytes` has the ABI version this runtime supports.
///
/// Reads the `ferricel.abi-version` custom section with [`crate::inspect`].
/// This runs only when the caller supplies raw Wasm bytes
/// ([`Builder::with_wasm`]). It does not run when the caller supplies a
/// pre-compiled [`wasmtime::Module`] ([`Builder::with_module`]), because the
/// raw bytes are not available at that point. A caller who uses
/// `with_module` and wants the same check can call
/// [`crate::abi_version`] on the bytes before compiling the module.
fn check_abi_version(wasm_bytes: &[u8]) -> Result<(), anyhow::Error> {
    let info = crate::inspect::inspect(wasm_bytes)?;
    match info.abi_version {
        Some(v) if v == ferricel_types::ABI_VERSION => Ok(()),
        Some(v) => Err(anyhow::anyhow!(
            "module ABI version {v} is not supported by this runtime (ABI version {}); \
             recompile the module with a matching ferricel version",
            ferricel_types::ABI_VERSION
        )),
        None => {
            let compiled_by = info
                .producers
                .iter()
                .find(|f| f.name == "processed-by")
                .and_then(|f| f.values.iter().find(|v| v.name == "ferricel"))
                .map(|v| format!(" (compiled by ferricel {})", v.version))
                .unwrap_or_default();
            Err(anyhow::anyhow!(
                "module has no ferricel.abi-version section{compiled_by}; this runtime \
                 supports ABI version {}; recompile the module with a matching ferricel version",
                ferricel_types::ABI_VERSION
            ))
        }
    }
}

/// Host state that holds data accessible to Wasm host functions.
struct HostState {
    logger: slog::Logger,
    /// Registered extension function implementations, keyed by (namespace, function).
    extensions: Extensions,
    /// Resource limits enforced on the instance's linear memories and tables.
    limits: wasmtime::StoreLimits,
}

/// Configure limits on the resources a single evaluation's Wasm instance is
/// allowed to consume, leveraging wasmtime's
/// [`ResourceLimiter`](wasmtime::ResourceLimiter) facility (via
/// [`wasmtime::StoreLimits`]).
///
/// This can be used to prevent a malicious, or misbehaving, compiled CEL
/// module from exhausting the host's memory, for example by growing its
/// linear memory in an unbounded loop.
///
/// When a limit is exceeded, the corresponding `memory.grow`/`table.grow`
/// Wasm instruction fails and returns `-1` to the guest, following the
/// WebAssembly specification. The ferricel guest runtime treats a failed
/// allocation as a fatal error and aborts, which is reported back to the
/// host as a trap (i.e. [`Engine::eval`] returns `Err`). The memory/table cap
/// itself is always enforced by the host regardless of how the guest reacts
/// to the failed growth.
///
/// Configure via [`Builder::with_resource_limits`].
#[derive(Clone, Copy, Debug, Default)]
pub struct ResourceLimits {
    /// Maximum size, in bytes, that the module's linear memory is allowed to
    /// grow to.
    ///
    /// `None` (the default) means no limit is enforced.
    pub max_memory_size: Option<usize>,

    /// Maximum number of elements the module's tables are allowed to grow
    /// to. This limit is applied to each table individually.
    ///
    /// Compiled CEL modules do not grow tables at runtime, so this is a
    /// secondary, defense-in-depth limit compared to
    /// [`ResourceLimits::max_memory_size`].
    ///
    /// `None` (the default) means no limit is enforced.
    pub max_table_elements: Option<usize>,
}

/// Build a `wasmtime::StoreLimits` out of the (optional) `ResourceLimits`
/// configuration. When `None` is provided, the resulting limits are
/// effectively unlimited (i.e. wasmtime's defaults).
fn store_limits(resource_limits: Option<ResourceLimits>) -> wasmtime::StoreLimits {
    let mut builder = wasmtime::StoreLimitsBuilder::new();
    if let Some(limits) = resource_limits {
        if let Some(max_memory_size) = limits.max_memory_size {
            builder = builder.memory_size(max_memory_size);
        }
        if let Some(max_table_elements) = limits.max_table_elements {
            builder = builder.table_elements(max_table_elements);
        }
    }
    builder.build()
}

/// Builder for configuring and constructing an [`Engine`].
///
/// All builder methods are consuming (take and return `Self`).
/// Call [`Builder::build`] to obtain an immutable [`Engine`].
///
/// [`Builder::build`] is fallible: it parses the Wasm bytes and pre-links all
/// host functions so that each call to [`Engine::eval`] only needs to
/// instantiate the pre-linked module, not recompile it.
///
/// # Example
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use ferricel_core::{compiler, runtime};
///
/// let wasm = compiler::Builder::new().build().compile("x * 2 + 1")?;
///
/// let result = runtime::Builder::new()
///     .with_wasm(wasm)
///     .build()?
///     .eval(Some(r#"{"x": 10}"#))?;
///
/// assert_eq!(result, "21");
/// # Ok(())
/// # }
/// ```
///
/// # Registering extension functions
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use ferricel_core::{compiler, runtime};
/// use ferricel_types::extensions::ExtensionDecl;
///
/// let abs_decl = ExtensionDecl {
///     namespace: None,
///     function: "abs".to_string(),
///     receiver_style: false,
///     global_style: true,
///     num_args: 1,
/// };
///
/// let wasm = compiler::Builder::new()
///     .with_extension(abs_decl.clone())
///     .build()
///     .compile("abs(x)")?;
///
/// let result = runtime::Builder::new()
///     .with_extension(abs_decl, |args| {
///         let n = args[0].as_i64().unwrap_or(0);
///         Ok(serde_json::Value::Number(n.abs().into()))
///     })
///     .with_wasm(wasm)
///     .build()?
///     .eval(Some(r#"{"x": -42}"#))?;
///
/// assert_eq!(result, "42");
/// # Ok(())
/// # }
/// ```
///
/// The runtime makes sure that each call has `decl.num_args` arguments
/// before it calls the closure. If a Wasm module sends the wrong count, the
/// call becomes a CEL evaluation error. The closure never reads `args` out
/// of bounds. See [`Extensions`] for details.
///
/// # Reuse extensions across evaluations with `EnginePre`
///
/// [`Builder::build_pre`] links the Wasm module without extension
/// implementations or a logger. For each request, call
/// [`EnginePre::rehydrate`] with an [`Extensions`] value to get an
/// [`Engine`]. The module is not compiled again.
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use ferricel_core::runtime::{self, Extensions};
/// use ferricel_core::compiler;
/// use ferricel_types::extensions::ExtensionDecl;
///
/// let abs_decl = ExtensionDecl {
///     namespace: None,
///     function: "abs".to_string(),
///     receiver_style: false,
///     global_style: true,
///     num_args: 1,
/// };
///
/// let wasm = compiler::Builder::new()
///     .with_extension(abs_decl.clone())
///     .build()
///     .compile("abs(x)")?;
///
/// let engine_pre = runtime::Builder::new().with_wasm(wasm).build_pre()?;
///
/// let mut extensions = Extensions::new();
/// extensions.register(abs_decl, |args| {
///     let n = args[0].as_i64().unwrap_or(0);
///     Ok(serde_json::Value::Number(n.abs().into()))
/// });
///
/// let logger = slog::Logger::root(slog::Discard, slog::o!());
/// let result = engine_pre
///     .rehydrate(extensions, logger, None)
///     .eval(Some(r#"{"x": -42}"#))?;
///
/// assert_eq!(result, "42");
/// # Ok(())
/// # }
/// ```
///
/// # Providing a custom wasmtime engine
///
/// By default [`build`](Self::build) creates a [`wasmtime::Engine`] with
/// default settings. Supply your own via [`with_engine`](Self::with_engine)
/// when you need custom [`wasmtime::Config`] options (fuel metering, epoch
/// interruption, etc.) or want to share a single compiled engine across
/// multiple [`Engine`] instances.
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use ferricel_core::{compiler, runtime};
/// use wasmtime::{Config, Engine as WasmEngine};
///
/// let config = Config::new();
///
/// let wasm_engine = WasmEngine::new(&config)?;
/// let wasm = compiler::Builder::new().build().compile("1 + 1")?;
///
/// let result = runtime::Builder::new()
///     .with_engine(wasm_engine)
///     .with_wasm(wasm)
///     .build()?
///     .eval(None)?;
/// # Ok(())
/// # }
/// ```
///
/// ## Epoch-based interruption
///
/// To bound evaluation time, enable `Config::epoch_interruption` on the
/// supplied engine and pair it with [`with_epoch_deadline`](Self::with_epoch_deadline).
/// The deadline is expressed in ticks beyond the current epoch; an embedder
/// thread must call `wasmtime::Engine::increment_epoch()` periodically for
/// the deadline to be reached and the evaluation to trap.
///
/// **Warning:** if `epoch_interruption` is enabled but no deadline is set
/// (via `with_epoch_deadline`), evaluation traps immediately — this is
/// `wasmtime`'s documented behavior for a `Store` with no configured
/// deadline on an interruption-enabled engine.
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use ferricel_core::{compiler, runtime};
/// use wasmtime::{Config, Engine as WasmEngine};
///
/// let mut config = Config::new();
/// config.epoch_interruption(true);
///
/// let wasm_engine = WasmEngine::new(&config)?;
/// let wasm = compiler::Builder::new().build().compile("1 + 1")?;
///
/// // No ticker thread is started here, so the deadline is never reached
/// // and this evaluation completes normally.
/// let result = runtime::Builder::new()
///     .with_engine(wasm_engine)
///     .with_epoch_deadline(1)
///     .with_wasm(wasm)
///     .build()?
///     .eval(None)?;
/// # Ok(())
/// # }
/// ```
///
/// ## Resource limits
///
/// To bound the amount of linear memory (and table elements) a single
/// evaluation is allowed to allocate, use
/// [`with_resource_limits`](Self::with_resource_limits). This protects the
/// host from a malicious or misbehaving CEL expression that keeps growing
/// memory (e.g. building up huge strings or lists in a comprehension).
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use ferricel_core::{compiler, runtime};
/// use ferricel_core::runtime::ResourceLimits;
///
/// let wasm = compiler::Builder::new().build().compile("1 + 1")?;
///
/// let result = runtime::Builder::new()
///     .with_resource_limits(ResourceLimits {
///         max_memory_size: Some(64 * 1024 * 1024), // 64 MiB
///         ..Default::default()
///     })
///     .with_wasm(wasm)
///     .build()?
///     .eval(None)?;
/// # Ok(())
/// # }
/// ```
pub struct Builder {
    logger: slog::Logger,
    log_level: LogLevel,
    extensions: Extensions,
    wasm_bytes: Option<Vec<u8>>,
    wasm_module: Option<Module>,
    wasm_engine: Option<WasmEngine>,
    epoch_deadline: Option<u64>,
    resource_limits: Option<ResourceLimits>,
}

impl Builder {
    /// Create a new builder with sensible defaults.
    ///
    /// The default logger discards all output. Override it with
    /// [`with_logger`](Self::with_logger) if you need log output.
    /// The default log level is [`LogLevel::Error`].
    pub fn new() -> Self {
        Self {
            logger: slog::Logger::root(slog::Discard, slog::o!()),
            log_level: LogLevel::Error,
            extensions: Extensions::new(),
            wasm_bytes: None,
            wasm_module: None,
            wasm_engine: None,
            epoch_deadline: None,
            resource_limits: None,
        }
    }

    /// Override the logger used during execution.
    pub fn with_logger(mut self, logger: slog::Logger) -> Self {
        self.logger = logger;
        self
    }

    /// Set the log level used during execution.
    pub fn with_log_level(mut self, level: LogLevel) -> Self {
        self.log_level = level;
        self
    }

    /// Register a host-provided extension function.
    ///
    /// The runtime uses `decl` for two things. It builds the
    /// `(namespace, function)` dispatch key from it. It also makes sure that
    /// each `cel_call_extension` request has `decl.num_args` arguments before
    /// it calls `implementation`. So `implementation` can trust
    /// `args.len() == decl.num_args`. For compile-time checks of count and
    /// call style, pass the same `decl` to
    /// [`crate::compiler::Builder::with_extension`].
    ///
    /// May be called multiple times to register several extensions.
    ///
    /// See the [Host Extensions](https://flavio.github.io/ferricel/host-extensions.html)
    /// user guide for details.
    pub fn with_extension(
        mut self,
        decl: ExtensionDecl,
        implementation: impl Fn(Vec<serde_json::Value>) -> Result<serde_json::Value, String>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.extensions.register(decl, implementation);
        self
    }

    /// Set an [`ExtensionAuthorizer`] on the extension set.
    ///
    /// The runtime calls `authorizer` with the `(namespace, function)` of
    /// every extension call. It calls `authorizer` after it finds the
    /// extension and before it parses the arguments. `Err(msg)` rejects
    /// the call. The guest receives `msg` as a CEL runtime error, with the
    /// extension as its origin. Use this method to reject a call that the
    /// current context does not allow, before the runtime spends work on
    /// the arguments.
    ///
    /// See the [Host Extensions](https://flavio.github.io/ferricel/host-extensions.html)
    /// user guide for details.
    pub fn with_extension_authorizer(
        mut self,
        authorizer: impl Fn(&ExtensionKey) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        self.extensions.set_extension_authorizer(authorizer);
        self
    }

    /// Provide a pre-configured [`wasmtime::Engine`] to use during compilation
    /// and execution.
    ///
    /// This is useful when you need non-default wasmtime settings (e.g. custom
    /// [`wasmtime::Config`] flags, fuel, epoch interruption, etc.) or when you
    /// want to share a single compiled [`wasmtime::Engine`] across multiple
    /// [`Engine`] instances.
    ///
    /// If this method is not called, [`build`](Self::build) creates a
    /// [`wasmtime::Engine`] with default settings via [`wasmtime::Engine::default`].
    pub fn with_engine(mut self, engine: WasmEngine) -> Self {
        self.wasm_engine = Some(engine);
        self
    }

    /// Set an epoch deadline (in ticks beyond the current epoch) applied to
    /// the [`wasmtime::Store`] created for each evaluation.
    ///
    /// This only has an effect when combined with a [`wasmtime::Engine`]
    /// supplied via [`with_engine`](Self::with_engine) that has
    /// `Config::epoch_interruption(true)` set, and requires the embedder to
    /// periodically call `wasmtime::Engine::increment_epoch()` (e.g. from a
    /// background thread) to actually trigger interruption. If the engine
    /// does not have epoch interruption enabled, setting a deadline is a
    /// no-op.
    pub fn with_epoch_deadline(mut self, ticks: u64) -> Self {
        self.epoch_deadline = Some(ticks);
        self
    }

    /// Enable enforcement of resource limits on the Wasm instance created for
    /// each evaluation, leveraging wasmtime's
    /// [`ResourceLimiter`](wasmtime::ResourceLimiter) facility.
    ///
    /// This can be used to prevent a malicious, or misbehaving, CEL
    /// expression from exhausting the host's memory. See [`ResourceLimits`]
    /// for details.
    pub fn with_resource_limits(mut self, resource_limits: ResourceLimits) -> Self {
        self.resource_limits = Some(resource_limits);
        self
    }

    /// Set the compiled Wasm bytes to execute.
    ///
    /// These bytes are parsed and pre-linked during [`build`](Self::build), so
    /// invalid Wasm is rejected eagerly rather than on the first [`eval`](Engine::eval) call.
    ///
    /// [`build_pre`](Self::build_pre) also checks the module's ABI version
    /// against [`ferricel_types::ABI_VERSION`] and returns `Err` on a
    /// mismatch or a missing `ferricel.abi-version` section.
    pub fn with_wasm(mut self, bytes: Vec<u8>) -> Self {
        self.wasm_bytes = Some(bytes);
        self
    }

    /// Provide a pre-compiled [`wasmtime::Module`] to execute.
    ///
    /// Use this when the caller has already compiled the module (e.g. via
    /// [`wasmtime::Module::from_file`]) and wants to avoid re-parsing the Wasm
    /// binary. A [`wasmtime::Engine`] must be supplied via
    /// [`with_engine`](Self::with_engine) and must be the same engine used to
    /// compile the module.
    ///
    /// Takes priority over [`with_wasm`](Self::with_wasm) when both are set.
    ///
    /// **This path skips the ABI version check** that [`with_wasm`](Self::with_wasm)
    /// runs, because the raw bytes are no longer available once a
    /// [`wasmtime::Module`] is compiled. Call [`crate::abi_version`] on the
    /// bytes yourself before compiling the module, if you need the check.
    pub fn with_module(mut self, module: Module) -> Self {
        self.wasm_module = Some(module);
        self
    }

    /// Consume the builder and produce an [`EnginePre`].
    ///
    /// This creates (or reuses) a [`wasmtime::Engine`], resolves the Wasm module
    /// (compiling from bytes if needed), registers all host functions into a
    /// [`Linker`], and calls [`Linker::instantiate_pre`] to produce a
    /// pre-linked [`wasmtime::InstancePre`].
    ///
    /// The resulting [`EnginePre`] can be cloned cheaply (all internals are
    /// `Arc`-backed) and rehydrated into a ready-to-use [`Engine`] at any time
    /// via [`EnginePre::rehydrate`], which is where per-evaluation-context state
    /// (e.g. extension function implementations) is injected.
    ///
    /// Returns `Err` if no Wasm was provided, if the module's ABI version
    /// does not match (see [`Builder::with_wasm`]), or if compilation/linking
    /// fails.
    pub fn build_pre(self) -> Result<EnginePre, anyhow::Error> {
        let wasm_engine = self.wasm_engine.unwrap_or_default();

        let module = if let Some(module) = self.wasm_module {
            module
        } else {
            let bytes = self.wasm_bytes.ok_or_else(|| {
                anyhow::anyhow!(
                    "no Wasm provided: call with_wasm() or with_module() before build_pre()"
                )
            })?;
            check_abi_version(&bytes)?;
            Module::from_binary(&wasm_engine, &bytes)?
        };

        let mut linker = Linker::<HostState>::new(&wasm_engine);
        Self::add_to_linker(&mut linker)?;

        let instance_pre = linker.instantiate_pre(&module)?;

        Ok(EnginePre {
            wasm_engine,
            instance_pre,
            log_level: self.log_level,
            resource_limits: self.resource_limits,
        })
    }

    /// Consume the builder and produce an immutable [`Engine`].
    ///
    /// Returns `Err` if no Wasm bytes were provided or if the bytes are invalid.
    pub fn build(self) -> Result<Engine, anyhow::Error> {
        let extensions = self.extensions.clone();
        let logger = self.logger.clone();
        let epoch_deadline = self.epoch_deadline;
        let pre = self.build_pre()?;
        Ok(pre.rehydrate(extensions, logger, epoch_deadline))
    }

    /// Register all host functions into the linker.
    fn add_to_linker(linker: &mut Linker<HostState>) -> Result<(), anyhow::Error> {
        Self::register_cel_log(linker)?;
        Self::register_cel_abort(linker)?;
        Self::register_cel_call_extension(linker)?;
        Ok(())
    }

    fn register_cel_log(linker: &mut Linker<HostState>) -> Result<(), anyhow::Error> {
        linker.func_wrap(
            "env",
            "cel_log",
            |mut caller: Caller<'_, HostState>,
             ptr: i32,
             len: i32|
             -> Result<(), wasmtime::Error> {
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .ok_or_else(|| wasmtime::Error::msg("Failed to get Wasm memory"))?;

                let buffer =
                    read_guest_bytes(&memory, &caller, ptr as u32 as usize, len as u32 as usize)?;

                let event: ferricel_types::LogEvent =
                    serde_json::from_slice(&buffer).map_err(|e| {
                        wasmtime::error::format_err!("Failed to deserialize log event: {}", e)
                    })?;

                let extra_json =
                    serde_json::to_string(&event.extra).unwrap_or_else(|_| "{}".to_string());

                let logger = &caller.data().logger;
                let child_logger = logger.new(slog::o!(
                    "file" => event.file,
                    "line" => event.line,
                    "column" => event.column,
                    "extra" => extra_json
                ));

                match event.level {
                    ferricel_types::LogLevel::Error => {
                        slog::error!(child_logger, "{}", event.message)
                    }
                    ferricel_types::LogLevel::Warn => {
                        slog::warn!(child_logger, "{}", event.message)
                    }
                    ferricel_types::LogLevel::Info => {
                        slog::info!(child_logger, "{}", event.message)
                    }
                    ferricel_types::LogLevel::Debug => {
                        slog::debug!(child_logger, "{}", event.message)
                    }
                }

                Ok(())
            },
        )?;
        Ok(())
    }

    fn register_cel_abort(linker: &mut Linker<HostState>) -> Result<(), anyhow::Error> {
        // The guest runtime calls this when a runtime error occurs (divide by
        // zero, overflow, a failed extension call, etc.).
        // The packed parameter contains: lower 32 bits = pointer, upper 32 bits = length.
        // The bytes hold a JSON-encoded `CelRuntimeError`.
        linker.func_wrap(
            "env",
            "cel_abort",
            |mut caller: Caller<'_, HostState>, packed: i64| -> Result<(), wasmtime::Error> {
                let address = (packed & 0xFFFFFFFF) as u32;
                let length = ((packed as u64) >> 32) as u32;

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .ok_or_else(|| wasmtime::Error::msg("Failed to get Wasm memory for error"))?;

                let buffer = read_guest_bytes(&memory, &caller, address as usize, length as usize)?;

                let error = parse_abort_payload(&buffer)?;

                // `Error::new` keeps the concrete type. The caller of
                // `Engine::eval` can get it back with
                // `err.downcast_ref::<CelRuntimeError>()`.
                Err(wasmtime::Error::new(error))
            },
        )?;
        Ok(())
    }

    fn register_cel_call_extension(linker: &mut Linker<HostState>) -> Result<(), anyhow::Error> {
        // The guest calls this to invoke a host-provided extension function.
        // packed: low 32 bits = ptr to request JSON, high 32 bits = len
        // returns: low 32 bits = ptr to response JSON, high 32 bits = len
        linker.func_wrap(
            "env",
            "cel_call_extension",
            |mut caller: Caller<'_, HostState>, packed: i64| -> Result<i64, wasmtime::Error> {
                let req_ptr = (packed & 0xFFFFFFFF) as u32 as usize;
                let req_len = (packed >> 32) as u32 as usize;

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .ok_or_else(|| wasmtime::Error::msg("Failed to get Wasm memory"))?;

                let req_buf = read_guest_bytes(&memory, &caller, req_ptr, req_len)?;

                // A malformed payload is a bug in the guest, not a CEL
                // error. It traps the instance. Every other failure goes
                // back to the guest as `ExtensionCallResponse::Error`. The
                // guest reports it as a CEL runtime error.
                let response = match dispatch_extension(&caller.data().extensions, &req_buf) {
                    Ok(v) => ExtensionCallResponse::Ok(v),
                    Err(DispatchError::Runtime(msg)) => ExtensionCallResponse::Error(msg),
                    Err(DispatchError::Malformed(msg)) => {
                        return Err(wasmtime::Error::msg(format!(
                            "Failed to deserialize extension payload: {}",
                            msg
                        )));
                    }
                };

                let resp_json = serde_json::to_vec(&response).unwrap_or_else(|e| {
                    serde_json::to_vec(&ExtensionCallResponse::Error(format!(
                        "Failed to serialize extension result: {}",
                        e
                    )))
                    .expect("serializing ExtensionCallResponse::Error never fails")
                });

                let resp_len = resp_json.len() as i32;
                let cel_malloc = caller
                    .get_export("cel_malloc")
                    .and_then(|e| e.into_func())
                    .ok_or_else(|| wasmtime::Error::msg("Failed to get cel_malloc export"))?
                    .typed::<i32, i32>(&caller)?;

                let resp_ptr = cel_malloc.call(&mut caller, resp_len)?;

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .ok_or_else(|| wasmtime::Error::msg("Failed to get Wasm memory"))?;
                memory.write(&mut caller, resp_ptr as usize, &resp_json)?;

                let encoded = (resp_ptr as i64) | ((resp_len as i64) << 32);
                Ok(encoded)
            },
        )?;
        Ok(())
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// A pre-linked, ready-to-rehydrate CEL engine.
///
/// Created via [`Builder::build_pre`]. Contains the pre-compiled
/// [`wasmtime::InstancePre`] but no extension function implementations yet.
///
/// Clone is cheap: both [`wasmtime::Engine`] and [`wasmtime::InstancePre`] are
/// internally `Arc`-backed.
///
/// Call [`EnginePre::rehydrate`] to produce a ready-to-use [`Engine`], injecting
/// per-evaluation-context state (extension function implementations and logger)
/// at that point.
#[derive(Clone)]
pub struct EnginePre {
    wasm_engine: WasmEngine,
    instance_pre: InstancePre<HostState>,
    log_level: LogLevel,
    resource_limits: Option<ResourceLimits>,
}

impl EnginePre {
    /// Produce an [`Engine`] by injecting per-evaluation-context state.
    ///
    /// The extension function implementations, the `logger`, and the
    /// `epoch_deadline` are supplied here — not at `build_pre` time — so
    /// callers can attach request-scoped context (e.g. a policy identifier,
    /// or a per-request timeout) at evaluation time rather than when the
    /// Wasm module was compiled and linked.
    ///
    /// This is infallible: all fallible work (compilation, linking,
    /// pre-instantiation) was done in [`Builder::build_pre`].
    ///
    /// Pass [`Extensions::new`] if the policy uses no extension functions.
    ///
    /// `epoch_deadline` sets the number of ticks (beyond the current epoch)
    /// after which the [`wasmtime::Store`] created for each evaluation will
    /// trap, when combined with a [`wasmtime::Engine`] that has
    /// `Config::epoch_interruption(true)` set (see
    /// [`Builder::with_engine`](crate::runtime::Builder::with_engine)). The
    /// embedder must periodically call `wasmtime::Engine::increment_epoch()`
    /// for the deadline to ever be reached.
    ///
    /// **Warning:** if the underlying `wasmtime::Engine` has epoch
    /// interruption enabled and `epoch_deadline` is `None`, every evaluation
    /// traps immediately — this is `wasmtime`'s documented behavior for a
    /// `Store` with no configured deadline on an interruption-enabled engine.
    /// Pass `Some(_)` whenever the engine has epoch interruption enabled.
    ///
    /// Passing `None` for an engine without epoch interruption enabled is a
    /// no-op (there is nothing to interrupt).
    pub fn rehydrate(
        &self,
        extensions: Extensions,
        logger: slog::Logger,
        epoch_deadline: Option<u64>,
    ) -> Engine {
        Engine {
            wasm_engine: self.wasm_engine.clone(),
            instance_pre: self.instance_pre.clone(),
            extensions_impl: extensions,
            logger,
            log_level: self.log_level,
            epoch_deadline,
            resource_limits: self.resource_limits,
        }
    }
}

/// An immutable CEL engine that evaluates a compiled Wasm module with optional
/// variable bindings and host-provided extension functions.
///
/// Construct via [`Builder`].
///
/// The underlying [`wasmtime::Engine`] and pre-linked [`wasmtime::InstancePre`]
/// are created once at [`Builder::build`] time and reused across every [`eval`](Engine::eval)
/// call, so per-call cost is limited to instantiation and evaluation.
///
/// # Example
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use ferricel_core::{compiler, runtime};
///
/// let compiler = compiler::Builder::new().build();
/// let wasm = compiler.compile("x * 2 + 1")?;
///
/// let result = runtime::Builder::new()
///     .with_wasm(wasm)
///     .build()?
///     .eval(Some(r#"{"x": 10}"#))?;
///
/// assert_eq!(result, "21");
/// # Ok(())
/// # }
/// ```
pub struct Engine {
    wasm_engine: WasmEngine,
    instance_pre: InstancePre<HostState>,
    /// Implementation map used during evaluation.
    extensions_impl: Extensions,
    /// Logger used for evaluation.
    logger: slog::Logger,
    /// Log level used during evaluation.
    log_level: LogLevel,
    /// Optional epoch deadline (in ticks) applied to each evaluation's Store.
    epoch_deadline: Option<u64>,
    /// Optional resource limits applied to each evaluation's Store.
    resource_limits: Option<ResourceLimits>,
}

impl Engine {
    /// Shared implementation for [`eval`](Self::eval) and [`eval_proto`](Self::eval_proto).
    ///
    /// `bindings_bytes` is the already-serialised bindings payload (JSON or protobuf).
    /// `export_name` is the Wasm export to call (`"evaluate"` or `"evaluate_proto"`).
    fn eval_raw(&self, bindings_bytes: &[u8], export_name: &str) -> Result<String, anyhow::Error> {
        let host_state = HostState {
            logger: self.logger.clone(),
            extensions: self.extensions_impl.clone(),
            limits: store_limits(self.resource_limits),
        };
        let mut store = Store::new(&self.wasm_engine, host_state);
        store.limiter(|s| &mut s.limits);
        if let Some(deadline) = self.epoch_deadline {
            store.set_epoch_deadline(deadline);
        }
        let instance = self.instance_pre.instantiate(&mut store)?;

        let cel_set_log_level = instance
            .get_typed_func::<i32, ()>(&mut store, "cel_set_log_level")
            .map_err(|e| anyhow::anyhow!("Failed to get 'cel_set_log_level' function: {}", e))?;
        cel_set_log_level.call(&mut store, self.log_level.as_i32())?;

        let cel_malloc = instance
            .get_typed_func::<i32, i32>(&mut store, "cel_malloc")
            .map_err(|e| anyhow::anyhow!("Failed to get 'cel_malloc' function: {}", e))?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow::anyhow!("Failed to get Wasm memory"))?;

        let len = bindings_bytes.len() as i32;
        let ptr = cel_malloc.call(&mut store, len)?;
        memory.write(&mut store, ptr as usize, bindings_bytes)?;
        let bindings_encoded = (ptr as i64) | ((len as i64) << 32);

        let evaluate = instance
            .get_typed_func::<i64, i64>(&mut store, export_name)
            .map_err(|e| anyhow::anyhow!("Failed to get '{}' function: {}", export_name, e))?;

        let encoded_result = evaluate.call(&mut store, bindings_encoded)?;

        let ptr = (encoded_result & 0xFFFFFFFF) as u32;
        let len = (encoded_result >> 32) as u32;
        let json_bytes = read_guest_bytes(&memory, &store, ptr as usize, len as usize)?;

        String::from_utf8(json_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to parse result as UTF-8: {}", e))
    }

    /// Evaluate the compiled Wasm module with optional JSON-encoded variable bindings.
    ///
    /// Extension implementations registered via [`Builder::with_extension`] are
    /// dispatched when the Wasm program calls an extension function.
    ///
    /// Returns a JSON-encoded CEL value string, or `Err` if the evaluation
    /// failed.
    ///
    /// # Errors
    ///
    /// If the CEL expression produced a runtime error, the returned error
    /// downcasts to [`CelRuntimeError`]. Its `origin` field is `Some` when a
    /// host extension produced the error. Other failures (an epoch-deadline
    /// interrupt, a memory-limit abort, a Wasm trap, a missing export, a bug
    /// in a host extension) do not downcast to [`CelRuntimeError`]. See the
    /// [module docs](self#runtime-errors).
    pub fn eval(&self, bindings_json: Option<&str>) -> Result<String, anyhow::Error> {
        self.eval_raw(bindings_json.unwrap_or("{}").as_bytes(), "evaluate")
    }

    /// Evaluate the compiled Wasm module with protobuf-encoded variable bindings.
    ///
    /// Unlike [`Engine::eval`], this method accepts a pre-encoded
    /// `ferricel.Bindings` protobuf message and calls the `evaluate_proto` export,
    /// which preserves full type fidelity for all CEL types (bytes, uint, timestamp,
    /// duration, etc.) that would be lost in a JSON round-trip.
    ///
    /// Returns a JSON-encoded CEL value string, or `Err` if the evaluation
    /// failed.
    ///
    /// # Errors
    ///
    /// Same as [`Engine::eval`]: a CEL runtime error downcasts to
    /// [`CelRuntimeError`], other failures do not.
    pub fn eval_proto(&self, bindings_proto: &[u8]) -> Result<String, anyhow::Error> {
        self.eval_raw(bindings_proto, "evaluate_proto")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    };

    use ferricel_types::extensions::ExtensionCallPayload;
    use rstest::rstest;

    use super::*;

    fn test_decl(num_args: usize) -> ExtensionDecl {
        ExtensionDecl {
            namespace: None,
            function: "myFunc".to_string(),
            receiver_style: false,
            global_style: true,
            num_args,
        }
    }

    /// The request bytes for a call to `myFunc` with `args`, in the wire
    /// format that the guest produces.
    fn payload(args: Vec<serde_json::Value>) -> Vec<u8> {
        serde_json::to_vec(&ExtensionCallPayload {
            namespace: None,
            function: "myFunc".to_string(),
            args,
        })
        .unwrap()
    }

    /// The request bytes for a call to `myFunc`, with `args_json` parsed
    /// and used as the `args` field. Use this function to build a payload
    /// whose `args` is valid JSON but not an array, something a real guest
    /// never sends.
    fn raw_payload(args_json: &str) -> Vec<u8> {
        let args: serde_json::Value = serde_json::from_str(args_json).unwrap();
        serde_json::to_vec(&serde_json::json!({
            "namespace": null,
            "function": "myFunc",
            "args": args,
        }))
        .unwrap()
    }

    /// Register `myFunc` with `num_args`, and a closure that records
    /// whether it ran.
    fn extensions_with_probe(num_args: usize) -> (Extensions, std::sync::Arc<AtomicBool>) {
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let mut extensions = Extensions::new();
        extensions.register(test_decl(num_args), move |_args| {
            called_clone.store(true, Ordering::SeqCst);
            Ok(serde_json::json!(42))
        });
        (extensions, called)
    }

    #[rstest]
    #[case::too_few(vec![], "expects 1 argument(s), got 0")]
    #[case::too_many(
        vec![serde_json::json!(1), serde_json::json!(2)],
        "expects 1 argument(s), got 2"
    )]
    fn dispatch_extension_rejects_wrong_arity_without_calling_closure(
        #[case] args: Vec<serde_json::Value>,
        #[case] expected_msg: &str,
    ) {
        let (extensions, called) = extensions_with_probe(1);

        let result = dispatch_extension(&extensions, &payload(args));

        assert!(!called.load(Ordering::SeqCst));
        let Err(DispatchError::Runtime(err)) = result else {
            panic!("expected Runtime error, got: {result:?}");
        };
        assert!(err.contains(expected_msg), "got: {err}");
    }

    #[test]
    fn dispatch_extension_calls_closure_when_arity_matches() {
        let mut extensions = Extensions::new();
        extensions.register(test_decl(2), |args| {
            Ok(serde_json::Value::Number(args.len().into()))
        });

        let result = dispatch_extension(
            &extensions,
            &payload(vec![serde_json::json!(1), serde_json::json!(2)]),
        );

        assert_eq!(result.unwrap(), serde_json::json!(2));
    }

    #[test]
    fn dispatch_extension_forwards_closure_error() {
        let mut extensions = Extensions::new();
        extensions.register(test_decl(0), |_args| Err("boom".to_string()));

        let result = dispatch_extension(&extensions, &payload(vec![]));

        assert_eq!(result, Err(DispatchError::Runtime("boom".to_string())));
    }

    #[test]
    fn dispatch_extension_unknown_key_is_an_error() {
        let extensions = Extensions::new();

        let result = dispatch_extension(&extensions, &payload(vec![]));

        let Err(DispatchError::Runtime(err)) = result else {
            panic!("expected Runtime error, got: {result:?}");
        };
        assert!(err.contains("Extension not found: myFunc"), "got: {err}");
    }

    #[rstest]
    #[case::not_json(b"not json".as_slice())]
    #[case::args_not_an_array(br#"{"namespace":null,"function":"myFunc","args":"not-an-array"}"#)]
    #[case::args_missing(br#"{"namespace":null,"function":"myFunc"}"#)]
    fn dispatch_extension_malformed_payload_is_malformed_error(#[case] req: &[u8]) {
        // A malformed payload is a bug in the guest. It must trap, not
        // become a CEL runtime error. The closure must not run.
        let (extensions, called) = extensions_with_probe(1);

        let result = dispatch_extension(&extensions, req);

        assert!(!called.load(Ordering::SeqCst));
        assert!(
            matches!(result, Err(DispatchError::Malformed(_))),
            "got: {result:?}"
        );
    }

    #[rstest]
    // If there is no authorizer, phase 2 returns `Malformed`.
    #[case::malformed_args(r#""not-an-array""#)]
    // If there is no authorizer, the arity check returns
    // `Runtime("myFunc expects 1 argument(s), got 2")`.
    #[case::wrong_arity("[1, 2]")]
    fn authorizer_denial_wins_over_later_checks(#[case] args_json: &str) {
        // Each case fails with a different error if the authorizer runs
        // later than planned. That is what proves the order: authorizer
        // first, then the `args` parse, then the arity check.
        let (mut extensions, called) = extensions_with_probe(1);
        extensions.set_extension_authorizer(|_key| Err("denied".to_string()));

        let result = dispatch_extension(&extensions, &raw_payload(args_json));

        assert!(!called.load(Ordering::SeqCst));
        assert_eq!(result, Err(DispatchError::Runtime("denied".to_string())));
    }

    #[test]
    fn authorizer_receives_namespace_and_function() {
        let seen: std::sync::Arc<Mutex<Option<ExtensionKey>>> = Default::default();
        let seen_clone = seen.clone();
        let decl = ExtensionDecl {
            namespace: Some("math".to_string()),
            function: "abs".to_string(),
            receiver_style: false,
            global_style: true,
            num_args: 1,
        };
        let extensions = Extensions::new()
            .with(decl, |_args| Ok(serde_json::Value::Null))
            .with_extension_authorizer(move |key| {
                *seen_clone.lock().unwrap() = Some(key.clone());
                Ok(())
            });
        let req = serde_json::to_vec(&ExtensionCallPayload {
            namespace: Some("math".to_string()),
            function: "abs".to_string(),
            args: vec![serde_json::json!(-1)],
        })
        .unwrap();

        dispatch_extension(&extensions, &req).unwrap();

        assert_eq!(
            *seen.lock().unwrap(),
            Some(ExtensionKey::new(
                Some("math".to_string()),
                "abs".to_string()
            ))
        );
    }

    #[test]
    fn authorizer_runs_after_unknown_extension_check() {
        // An unknown extension is a mismatch between the compiler and the
        // runtime, not an authorization question. The authorizer does not
        // run for this case.
        let authorizer_ran = std::sync::Arc::new(AtomicBool::new(false));
        let authorizer_ran_clone = authorizer_ran.clone();
        let extensions = Extensions::new().with_extension_authorizer(move |_key| {
            authorizer_ran_clone.store(true, Ordering::SeqCst);
            Err("denied".to_string())
        });

        let result = dispatch_extension(&extensions, &payload(vec![]));

        assert!(!authorizer_ran.load(Ordering::SeqCst));
        let Err(DispatchError::Runtime(err)) = result else {
            panic!("expected Runtime error, got: {result:?}");
        };
        assert!(err.contains("Extension not found: myFunc"), "got: {err}");
    }

    #[test]
    fn authorizer_ok_lets_call_through() {
        let (mut extensions, called) = extensions_with_probe(1);
        extensions.set_extension_authorizer(|_key| Ok(()));

        let result = dispatch_extension(&extensions, &payload(vec![serde_json::json!(1)]));

        assert!(called.load(Ordering::SeqCst));
        assert_eq!(result.unwrap(), serde_json::json!(42));
    }

    /// A one-page memory whose first bytes are `0..=9`.
    fn one_page_memory() -> (Store<()>, wasmtime::Memory) {
        let engine = WasmEngine::default();
        let mut store = Store::new(&engine, ());
        let memory =
            wasmtime::Memory::new(&mut store, wasmtime::MemoryType::new(1, Some(1))).unwrap();
        memory
            .write(&mut store, 0, &(0..10).collect::<Vec<u8>>())
            .unwrap();
        (store, memory)
    }

    const PAGE: usize = 65536;

    #[rstest]
    #[case::in_bounds(2, 5, Some(vec![2, 3, 4, 5, 6]))]
    #[case::empty_at_end(PAGE, 0, Some(vec![]))]
    #[case::len_past_end(PAGE - 4, 8, None)]
    #[case::ptr_past_end(PAGE, 1, None)]
    #[case::huge_len(0, u32::MAX as usize, None)]
    #[case::ptr_plus_len_overflows(usize::MAX, 2, None)]
    fn read_guest_bytes_checks_bounds_before_it_allocates(
        #[case] ptr: usize,
        #[case] len: usize,
        #[case] expected: Option<Vec<u8>>,
    ) {
        let (store, memory) = one_page_memory();

        let result = read_guest_bytes(&memory, &store, ptr, len);

        match expected {
            Some(bytes) => assert_eq!(result.unwrap(), bytes),
            None => {
                let err = result.unwrap_err().to_string();
                assert!(err.contains("out of bounds"), "got: {err}");
            }
        }
    }

    #[rstest]
    #[case::without_origin(CelRuntimeError::new("divide by zero"))]
    #[case::with_origin(CelRuntimeError::from_extension("not found", Some("kw.k8s"), "get"))]
    fn parse_abort_payload_round_trips_json(#[case] expected: CelRuntimeError) {
        let payload = serde_json::to_vec(&expected).unwrap();

        let err = parse_abort_payload(&payload).unwrap();

        assert_eq!(err, expected);
    }

    #[rstest]
    #[case::plain_text(b"divide by zero")]
    #[case::invalid_utf8(&[0xff, 0xfe])]
    #[case::wrong_shape(br#"{"error": "divide by zero"}"#)]
    fn parse_abort_payload_rejects_non_cel_runtime_error(#[case] bytes: &[u8]) {
        // The guest must send a JSON `CelRuntimeError`. Anything else is a
        // guest bug and must not look like a CEL runtime error.
        let err = parse_abort_payload(bytes).unwrap_err();

        assert!(
            err.to_string().contains("Invalid cel_abort payload"),
            "got: {err}"
        );
        let anyhow_err: anyhow::Error = err.into();
        assert!(anyhow_err.downcast_ref::<CelRuntimeError>().is_none());
    }

    #[test]
    fn cel_runtime_error_survives_wasmtime_and_anyhow_conversion() {
        // This is the path a guest abort takes: `wasmtime::Error::new` in the
        // host function, then `?` in `eval_raw` converts it to `anyhow::Error`.
        let original = CelRuntimeError::from_extension("boom", None::<String>, "f");
        let wasmtime_err = wasmtime::Error::new(original.clone());
        let anyhow_err: anyhow::Error = wasmtime_err.into();

        assert_eq!(anyhow_err.to_string(), "CEL runtime error: boom");
        assert_eq!(
            anyhow_err.downcast_ref::<CelRuntimeError>(),
            Some(&original)
        );
    }
}
