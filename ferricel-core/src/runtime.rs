use ferricel_types::LogLevel;
use ferricel_types::extensions::{ExtensionCallPayload, ExtensionDecl};
use wasmtime::*;

use crate::compiler::ExtensionKey;

/// Type alias for an extension function implementation.
pub type ExtensionFn = std::sync::Arc<
    dyn Fn(Vec<serde_json::Value>) -> Result<serde_json::Value, String> + Send + Sync,
>;

/// Host state that holds data accessible to WASM host functions
struct HostState {
    logger: slog::Logger,
    /// Registered extension function implementations keyed by (namespace, function).
    extensions: std::collections::HashMap<ExtensionKey, ExtensionFn>,
}

/// A high-level CEL engine that executes compiled WASM modules with optional
/// variable bindings and host-provided extension functions.
///
/// # Executing a compiled module
///
/// Pass the WASM bytes produced by `compile_cel_to_wasm` along with an optional
/// JSON bindings map. The result is a JSON-encoded CEL value.
///
/// ```rust,ignore
/// // CEL expression: "x * 2 + 1"
/// let wasm = compile_cel_to_wasm("x * 2 + 1", CompilerOptions::default())?;
///
/// let result = CelEngine::new(logger)
///     .execute(&wasm, Some(r#"{"x": 10}"#))?;
///
/// assert_eq!(result, "21");
/// ```
///
/// # Registering extension functions
///
/// CEL has no built-in `abs` for integers. You can provide it as an extension.
/// The same [`ferricel_types::extensions::ExtensionDecl`] must be given to the
/// compiler (so it can validate arity and call style) **and** to the engine
/// (so it knows which host function to call at runtime).
///
/// ```rust,ignore
/// use ferricel_types::extensions::ExtensionDecl;
///
/// // Declare abs(x: int) -> int as a global-style, single-argument extension.
/// let abs_decl = ExtensionDecl {
///     namespace: None,       // no namespace — called as abs(x), not myns.abs(x)
///     function: "abs".to_string(),
///     receiver_style: false, // not called as x.abs()
///     global_style: true,    // called as abs(x)
///     num_args: 1,
/// };
///
/// // 1. Compile time: pass the decl so the compiler accepts the abs() call.
/// // CEL expression: "abs(x)"
/// let wasm = compile_cel_to_wasm("abs(x)", CompilerOptions {
///     extensions: vec![abs_decl.clone()],
///     ..Default::default()
/// })?;
///
/// // 2. Runtime: register the host implementation under the same decl.
/// let mut engine = CelEngine::new(logger);
/// engine.register_extension(abs_decl, |args| {
///     let n = args[0].as_i64().unwrap_or(0);
///     Ok(serde_json::Value::Number(n.abs().into()))
/// });
///
/// let result = engine.execute(&wasm, Some(r#"{"x": -42}"#))?;
/// assert_eq!(result, "42");
/// ```
///
/// The same [`ferricel_types::extensions::ExtensionDecl`] must also be passed to
/// [`crate::compiler::CompilerOptions::extensions`] at compile time so the compiler
/// can validate call sites (arity, call style).
///
/// # Log level
///
/// The default log level is [`LogLevel::Error`]. Use [`CelEngine::with_log_level`] to
/// increase verbosity:
///
/// ```rust,ignore
/// use ferricel_types::LogLevel;
///
/// let engine = CelEngine::new(logger).with_log_level(LogLevel::Info);
/// ```
pub struct CelEngine {
    /// Implementation map used during execution.
    extensions_impl: std::collections::HashMap<ExtensionKey, ExtensionFn>,
    /// Logger used for execution.
    logger: slog::Logger,
    /// Log level used during execution.
    log_level: LogLevel,
}

impl CelEngine {
    /// Create a new engine with no extensions.
    pub fn new(logger: slog::Logger) -> Self {
        Self {
            extensions_impl: std::collections::HashMap::new(),
            logger,
            log_level: LogLevel::Error,
        }
    }

    /// Set the log level used during execution.
    pub fn with_log_level(mut self, level: LogLevel) -> Self {
        self.log_level = level;
        self
    }

    /// Register a host-provided extension function.
    ///
    /// The `decl` is used to derive the `(namespace, function)` key for dispatch
    /// at runtime. For compile-time arity/style validation, pass `decl` to
    /// [`crate::compiler::CompilerOptions::extensions`] when calling
    /// [`crate::compiler::compile_cel_to_wasm`].
    pub fn register_extension(
        &mut self,
        decl: ExtensionDecl,
        implementation: impl Fn(Vec<serde_json::Value>) -> Result<serde_json::Value, String>
        + Send
        + Sync
        + 'static,
    ) -> &mut Self {
        let key = ExtensionKey::new(decl.namespace.clone(), decl.function.clone());
        self.extensions_impl
            .insert(key, std::sync::Arc::new(implementation));
        self
    }

    /// Execute a compiled WASM module with optional variable bindings.
    ///
    /// Extension implementations registered with [`CelEngine::register_extension`] are
    /// dispatched when the WASM program calls an extension function.
    pub fn execute(
        &self,
        wasm_bytes: &[u8],
        bindings_json: Option<&str>,
    ) -> Result<String, anyhow::Error> {
        let extensions: std::collections::HashMap<ExtensionKey, ExtensionFn> = self
            .extensions_impl
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        execute_wasm_inner(
            wasm_bytes,
            bindings_json,
            self.log_level,
            self.logger.clone(),
            extensions,
        )
    }

    /// Execute a compiled WASM module with protobuf-encoded variable bindings.
    ///
    /// Unlike [`CelEngine::execute`], this method accepts a pre-encoded
    /// `ferricel.Bindings` protobuf message and calls the `validate_proto` export,
    /// which preserves full type fidelity for all CEL types (bytes, uint, timestamp,
    /// duration, etc.) that would be lost in a JSON round-trip.
    ///
    /// The result is still a JSON-encoded CEL value string.
    pub fn execute_proto(
        &self,
        wasm_bytes: &[u8],
        bindings_proto: &[u8],
    ) -> Result<String, anyhow::Error> {
        let extensions: std::collections::HashMap<ExtensionKey, ExtensionFn> = self
            .extensions_impl
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        execute_wasm_proto_inner(
            wasm_bytes,
            bindings_proto,
            self.log_level,
            self.logger.clone(),
            extensions,
        )
    }
}

/// Internal implementation: execute a compiled WASM module with variable bindings
/// and optional extension implementations.
fn execute_wasm_inner(
    wasm_bytes: &[u8],
    bindings_json: Option<&str>,
    log_level: LogLevel,
    logger: slog::Logger,
    extensions: std::collections::HashMap<ExtensionKey, ExtensionFn>,
) -> Result<String, anyhow::Error> {
    // Create a Wasmtime engine and store with host state
    let engine = Engine::default();
    let host_state = HostState { logger, extensions };
    let mut store = Store::new(&engine, host_state);

    // Load and compile the WASM module from bytes
    let module = Module::from_binary(&engine, wasm_bytes)?;

    // Create a linker and add the cel_log host function
    let mut linker = Linker::new(&engine);

    // Add cel_log host function for structured logging
    linker.func_wrap(
        "env",
        "cel_log",
        |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| -> Result<(), wasmtime::Error> {
            // Get the WASM memory
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Error::msg("Failed to get WASM memory"))?;

            // Read the JSON log event from WASM memory
            let mut buffer = vec![0u8; len as usize];
            memory.read(&caller, ptr as usize, &mut buffer)?;

            // Deserialize the log event using the shared LogEvent type
            let event: ferricel_types::LogEvent = serde_json::from_slice(&buffer).map_err(|e| {
                wasmtime::error::format_err!("Failed to deserialize log event: {}", e)
            })?;

            // Serialize extra KV pairs to JSON string
            let extra_json =
                serde_json::to_string(&event.extra).unwrap_or_else(|_| "{}".to_string());

            // Access the logger from the store's host state
            let logger = &caller.data().logger;

            // Create child logger with base KV pairs (file, line, column, extra)
            let child_logger = logger.new(slog::o!(
                "file" => event.file,
                "line" => event.line,
                "column" => event.column,
                "extra" => extra_json
            ));

            // Log with appropriate level
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

    // Add cel_abort host function for error handling
    // The guest runtime calls this when a runtime error occurs (divide by zero, overflow, etc.)
    // The packed parameter contains: upper 32 bits = address, lower 32 bits = length
    linker.func_wrap(
        "env",
        "cel_abort",
        |mut caller: Caller<'_, HostState>, packed: i64| -> Result<(), wasmtime::Error> {
            // Unpack address and length from the packed i64
            let address = ((packed as u64) >> 32) as u32;
            let length = packed as u32;

            // Get the WASM memory
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Error::msg("Failed to get WASM memory for error"))?;

            // Read error message from WASM memory
            let mut buffer = vec![0u8; length as usize];
            memory.read(&caller, address as usize, &mut buffer)?;

            // Convert to UTF-8 string
            let error_message = std::str::from_utf8(&buffer).map_err(|e| {
                wasmtime::Error::msg(format!("Invalid UTF-8 in error message: {}", e))
            })?;

            // Return an error to terminate WASM execution
            Err(wasmtime::Error::msg(format!(
                "CEL runtime error: {}",
                error_message
            )))
        },
    )?;

    // Add cel_call_extension host function for extension function dispatch.
    // The guest calls this to invoke a host-provided extension function.
    // packed: low 32 bits = ptr to request JSON, high 32 bits = len
    // returns: low 32 bits = ptr to response JSON, high 32 bits = len
    linker.func_wrap(
        "env",
        "cel_call_extension",
        |mut caller: Caller<'_, HostState>, packed: i64| -> Result<i64, wasmtime::Error> {
            // Unpack request pointer and length.
            let req_ptr = (packed & 0xFFFFFFFF) as u32 as usize;
            let req_len = (packed >> 32) as u32 as usize;

            // Read the request JSON from WASM memory.
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Error::msg("Failed to get WASM memory"))?;

            let mut req_buf = vec![0u8; req_len];
            memory.read(&caller, req_ptr, &mut req_buf)?;

            // Deserialize the request payload.
            let payload: ExtensionCallPayload = serde_json::from_slice(&req_buf).map_err(|e| {
                wasmtime::Error::msg(format!("Failed to deserialize extension payload: {}", e))
            })?;

            // Look up the extension implementation.
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

            // Convert the result to a JSON-serialized CelValue.
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

            // Allocate WASM memory for the response via cel_malloc.
            let resp_len = resp_json.len() as i32;
            let cel_malloc = caller
                .get_export("cel_malloc")
                .and_then(|e| e.into_func())
                .ok_or_else(|| wasmtime::Error::msg("Failed to get cel_malloc export"))?
                .typed::<i32, i32>(&caller)?;

            let resp_ptr = cel_malloc.call(&mut caller, resp_len)?;

            // Write the response JSON to WASM memory.
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Error::msg("Failed to get WASM memory"))?;
            memory.write(&mut caller, resp_ptr as usize, &resp_json)?;

            // Pack (ptr, len) and return.
            let encoded = (resp_ptr as i64) | ((resp_len as i64) << 32);
            Ok(encoded)
        },
    )?;

    // Create an instance using the linker
    let instance = linker.instantiate(&mut store, &module)?;

    // Set the log level in the WASM runtime before execution
    let cel_set_log_level = instance
        .get_typed_func::<i32, ()>(&mut store, "cel_set_log_level")
        .map_err(|e| anyhow::anyhow!("Failed to get 'cel_set_log_level' function: {}", e))?;

    cel_set_log_level.call(&mut store, log_level.as_i32())?;

    // Get the cel_malloc function to allocate memory in WASM
    let cel_malloc = instance
        .get_typed_func::<i32, i32>(&mut store, "cel_malloc")
        .map_err(|e| anyhow::anyhow!("Failed to get 'cel_malloc' function: {}", e))?;

    // Get the WASM memory
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| anyhow::anyhow!("Failed to get WASM memory"))?;

    // Helper function to allocate and write JSON to WASM memory
    let mut allocate_json = |json: &str| -> Result<i64, anyhow::Error> {
        let json_bytes = json.as_bytes();
        let len = json_bytes.len() as i32;

        // Allocate memory in WASM
        let ptr = cel_malloc.call(&mut store, len)?;

        // Write JSON bytes to WASM memory
        memory.write(&mut store, ptr as usize, json_bytes)?;

        // Encode (ptr, len) as i64: low 32 bits = ptr, high 32 bits = len
        let encoded = (ptr as i64) | ((len as i64) << 32);
        Ok(encoded)
    };

    // Encode bindings parameter (default to empty map if not provided)
    let bindings_encoded = if let Some(json) = bindings_json {
        allocate_json(json)?
    } else {
        allocate_json("{}")?
    };

    // Get the 'validate' function (now takes single bindings parameter)
    let validate = instance
        .get_typed_func::<i64, i64>(&mut store, "validate")
        .map_err(|e| anyhow::anyhow!("Failed to get 'validate' function: {}", e))?;

    // Call the validate function with encoded bindings
    let encoded_result = validate.call(&mut store, bindings_encoded)?;

    // Decode the pointer and length from the i64 result
    let ptr = (encoded_result & 0xFFFFFFFF) as u32;
    let len = (encoded_result >> 32) as u32;

    // Read the JSON bytes from WASM memory
    let mut json_bytes = vec![0u8; len as usize];
    memory.read(&store, ptr as usize, &mut json_bytes)?;

    // Convert bytes to UTF-8 string
    let json_string = String::from_utf8(json_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON as UTF-8: {}", e))?;

    Ok(json_string)
}

/// Internal implementation: execute a compiled WASM module with protobuf-encoded bindings.
/// Calls `validate_proto` instead of `validate`.
fn execute_wasm_proto_inner(
    wasm_bytes: &[u8],
    bindings_proto: &[u8],
    log_level: LogLevel,
    logger: slog::Logger,
    extensions: std::collections::HashMap<ExtensionKey, ExtensionFn>,
) -> Result<String, anyhow::Error> {
    // Set up engine, store, linker — identical to execute_wasm_inner
    let engine = Engine::default();
    let host_state = HostState { logger, extensions };
    let mut store = Store::new(&engine, host_state);

    let module = Module::from_binary(&engine, wasm_bytes)?;
    let mut linker = Linker::new(&engine);

    // cel_log — same as execute_wasm_inner
    linker.func_wrap(
        "env",
        "cel_log",
        |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| -> Result<(), wasmtime::Error> {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Error::msg("Failed to get WASM memory"))?;

            let mut buffer = vec![0u8; len as usize];
            memory.read(&caller, ptr as usize, &mut buffer)?;

            let event: ferricel_types::LogEvent = serde_json::from_slice(&buffer).map_err(|e| {
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

    // cel_abort — same as execute_wasm_inner
    linker.func_wrap(
        "env",
        "cel_abort",
        |mut caller: Caller<'_, HostState>, packed: i64| -> Result<(), wasmtime::Error> {
            let address = ((packed as u64) >> 32) as u32;
            let length = packed as u32;

            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Error::msg("Failed to get WASM memory for error"))?;

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

    // cel_call_extension — same as execute_wasm_inner
    linker.func_wrap(
        "env",
        "cel_call_extension",
        |mut caller: Caller<'_, HostState>, packed: i64| -> Result<i64, wasmtime::Error> {
            let req_ptr = (packed & 0xFFFFFFFF) as u32 as usize;
            let req_len = (packed >> 32) as u32 as usize;

            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| wasmtime::Error::msg("Failed to get WASM memory"))?;

            let mut req_buf = vec![0u8; req_len];
            memory.read(&caller, req_ptr, &mut req_buf)?;

            let payload: ExtensionCallPayload = serde_json::from_slice(&req_buf).map_err(|e| {
                wasmtime::Error::msg(format!("Failed to deserialize extension payload: {}", e))
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
                .ok_or_else(|| wasmtime::Error::msg("Failed to get WASM memory"))?;
            memory.write(&mut caller, resp_ptr as usize, &resp_json)?;

            let encoded = (resp_ptr as i64) | ((resp_len as i64) << 32);
            Ok(encoded)
        },
    )?;

    let instance = linker.instantiate(&mut store, &module)?;

    // Set log level
    let cel_set_log_level = instance
        .get_typed_func::<i32, ()>(&mut store, "cel_set_log_level")
        .map_err(|e| anyhow::anyhow!("Failed to get 'cel_set_log_level' function: {}", e))?;
    cel_set_log_level.call(&mut store, log_level.as_i32())?;

    let cel_malloc = instance
        .get_typed_func::<i32, i32>(&mut store, "cel_malloc")
        .map_err(|e| anyhow::anyhow!("Failed to get 'cel_malloc' function: {}", e))?;

    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| anyhow::anyhow!("Failed to get WASM memory"))?;

    // Allocate and write proto bytes into WASM memory
    let proto_len = bindings_proto.len() as i32;
    let proto_ptr = cel_malloc.call(&mut store, proto_len)?;
    memory.write(&mut store, proto_ptr as usize, bindings_proto)?;
    let bindings_encoded = (proto_ptr as i64) | ((proto_len as i64) << 32);

    // Call validate_proto
    let validate_proto = instance
        .get_typed_func::<i64, i64>(&mut store, "validate_proto")
        .map_err(|e| anyhow::anyhow!("Failed to get 'validate_proto' function: {}", e))?;

    let encoded_result = validate_proto.call(&mut store, bindings_encoded)?;

    // Decode result
    let ptr = (encoded_result & 0xFFFFFFFF) as u32;
    let len = (encoded_result >> 32) as u32;

    let mut json_bytes = vec![0u8; len as usize];
    memory.read(&store, ptr as usize, &mut json_bytes)?;

    String::from_utf8(json_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON as UTF-8: {}", e))
}
