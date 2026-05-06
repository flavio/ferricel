use ferricel_types::{
    LogLevel,
    extensions::{ExtensionCallPayload, ExtensionDecl},
};
use wasmtime::{Caller, Engine as WasmEngine, InstancePre, Linker, Module, Store};

use crate::compiler::ExtensionKey;

/// Type alias for an extension function implementation.
pub type ExtensionFn = std::sync::Arc<
    dyn Fn(Vec<serde_json::Value>) -> Result<serde_json::Value, String> + Send + Sync,
>;

/// Host state that holds data accessible to Wasm host functions.
struct HostState {
    logger: slog::Logger,
    /// Registered extension function implementations keyed by (namespace, function).
    extensions: std::collections::HashMap<ExtensionKey, ExtensionFn>,
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
pub struct Builder {
    logger: slog::Logger,
    log_level: LogLevel,
    extensions: std::collections::HashMap<ExtensionKey, ExtensionFn>,
    wasm_bytes: Option<Vec<u8>>,
    wasm_engine: Option<WasmEngine>,
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
            extensions: std::collections::HashMap::new(),
            wasm_bytes: None,
            wasm_engine: None,
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
    /// The `decl` is used to derive the `(namespace, function)` key for dispatch
    /// at runtime. For compile-time arity/style validation, pass the same `decl` to
    /// [`crate::compiler::Builder::with_extension`].
    ///
    /// May be called multiple times to register several extensions.
    pub fn with_extension(
        mut self,
        decl: ExtensionDecl,
        implementation: impl Fn(Vec<serde_json::Value>) -> Result<serde_json::Value, String>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        let key = ExtensionKey::new(decl.namespace.clone(), decl.function.clone());
        self.extensions
            .insert(key, std::sync::Arc::new(implementation));
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

    /// Set the compiled Wasm bytes to execute.
    ///
    /// These bytes are parsed and pre-linked during [`build`](Self::build), so
    /// invalid Wasm is rejected eagerly rather than on the first [`eval`](Engine::eval) call.
    pub fn with_wasm(mut self, bytes: Vec<u8>) -> Self {
        self.wasm_bytes = Some(bytes);
        self
    }

    /// Consume the builder and produce an immutable [`Engine`].
    ///
    /// This creates (or reuses) a [`wasmtime::Engine`], parses the Wasm module,
    /// registers all host functions, and calls [`Linker::instantiate_pre`] so that
    /// subsequent [`eval`](Engine::eval) calls only pay the cost of instantiation,
    /// not compilation.
    ///
    /// If no [`wasmtime::Engine`] was supplied via [`with_engine`](Self::with_engine),
    /// a default one is created via [`wasmtime::Engine::default`].
    ///
    /// Returns `Err` if no Wasm bytes were provided or if the bytes are invalid.
    pub fn build(self) -> Result<Engine, anyhow::Error> {
        let bytes = self.wasm_bytes.ok_or_else(|| {
            anyhow::anyhow!("no Wasm bytes provided: call with_wasm() before build()")
        })?;

        let wasm_engine = self.wasm_engine.unwrap_or_default();
        let module = Module::from_binary(&wasm_engine, &bytes)?;

        let mut linker = Linker::<HostState>::new(&wasm_engine);
        Self::add_to_linker(&mut linker)?;

        let instance_pre = linker.instantiate_pre(&module)?;

        Ok(Engine {
            wasm_engine,
            instance_pre,
            extensions_impl: self.extensions,
            logger: self.logger,
            log_level: self.log_level,
        })
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

                let mut buffer = vec![0u8; len as usize];
                memory.read(&caller, ptr as usize, &mut buffer)?;

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
        // The guest runtime calls this when a runtime error occurs (divide by zero, overflow, etc.)
        // The packed parameter contains: lower 32 bits = pointer, upper 32 bits = length.
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

                let mut buffer = vec![0u8; length as usize];
                memory.read(&caller, address as usize, &mut buffer)?;

                let error_message = std::str::from_utf8(&buffer).map_err(|e| {
                    wasmtime::Error::msg(format!("Invalid UTF-8 in error message: {}", e))
                })?;

                Err(wasmtime::Error::msg(format!(
                    "CEL runtime error: {}",
                    error_message
                )))
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

                let mut req_buf = vec![0u8; req_len];
                memory.read(&caller, req_ptr, &mut req_buf)?;

                let payload: ExtensionCallPayload =
                    serde_json::from_slice(&req_buf).map_err(|e| {
                        wasmtime::Error::msg(format!(
                            "Failed to deserialize extension payload: {}",
                            e
                        ))
                    })?;

                let key = ExtensionKey::new(payload.namespace.clone(), payload.function.clone());
                let result_value = {
                    let ext_fn = caller.data().extensions.get(&key);
                    match ext_fn {
                        Some(f) => f(payload.args.clone()),
                        None => {
                            let full_name = match &payload.namespace {
                                Some(ns) => format!("{}.{}", ns, payload.function),
                                None => payload.function.clone(),
                            };
                            Err(format!("Extension not found: {}", full_name))
                        }
                    }
                };

                let resp_json = match result_value {
                    Ok(v) => serde_json::to_vec(&v).unwrap_or_else(|e| {
                        format!(
                            r#"{{"error":"Failed to serialize extension result: {}"}}"#,
                            e
                        )
                        .into_bytes()
                    }),
                    Err(msg) => {
                        let escaped = msg.replace('"', "\\\"");
                        format!(r#"{{"error":"{}"}}"#, escaped).into_bytes()
                    }
                };

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
    extensions_impl: std::collections::HashMap<ExtensionKey, ExtensionFn>,
    /// Logger used for evaluation.
    logger: slog::Logger,
    /// Log level used during evaluation.
    log_level: LogLevel,
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
        };
        let mut store = Store::new(&self.wasm_engine, host_state);
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
        let mut json_bytes = vec![0u8; len as usize];
        memory.read(&store, ptr as usize, &mut json_bytes)?;

        String::from_utf8(json_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to parse result as UTF-8: {}", e))
    }

    /// Evaluate the compiled Wasm module with optional JSON-encoded variable bindings.
    ///
    /// Extension implementations registered via [`Builder::with_extension`] are
    /// dispatched when the Wasm program calls an extension function.
    ///
    /// Returns a JSON-encoded CEL value string, or `Err` if the expression
    /// produced a runtime error.
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
    /// Returns a JSON-encoded CEL value string, or `Err` if the expression
    /// produced a runtime error.
    pub fn eval_proto(&self, bindings_proto: &[u8]) -> Result<String, anyhow::Error> {
        self.eval_raw(bindings_proto, "evaluate_proto")
    }
}
