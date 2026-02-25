use ferricel_types::LogLevel;
use wasmtime::*;

/// Host state that holds data accessible to WASM host functions
struct HostState {
    logger: slog::Logger,
}

/// Execute a compiled WASM module with input and data variables
///
/// Takes the WASM module bytes and optional input/data JSON strings.
/// Allocates memory in WASM for the JSON, calls validate with encoded pointers,
/// and returns the JSON-serialized result as a String.
pub fn execute_wasm_with_vars(
    wasm_bytes: &[u8],
    input_json: Option<&str>,
    data_json: Option<&str>,
    log_level: LogLevel,
    logger: slog::Logger,
) -> Result<String, anyhow::Error> {
    // Create a Wasmtime engine and store with host state
    let engine = Engine::default();
    let host_state = HostState { logger };
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

    // Encode input and data parameters
    let input_encoded = if let Some(json) = input_json {
        allocate_json(json)?
    } else {
        0 // No input provided
    };

    let data_encoded = if let Some(json) = data_json {
        allocate_json(json)?
    } else {
        0 // No data provided
    };

    // Get the 'validate' function
    let validate = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "validate")
        .map_err(|e| anyhow::anyhow!("Failed to get 'validate' function: {}", e))?;

    // Call the validate function with encoded input and data
    let encoded_result = validate.call(&mut store, (input_encoded, data_encoded))?;

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
