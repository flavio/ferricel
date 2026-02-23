use wasmtime::*;

/// Execute a compiled WASM module with input and data variables
///
/// Takes the WASM module bytes and optional input/data JSON strings.
/// Allocates memory in WASM for the JSON, calls validate with encoded pointers,
/// and returns the JSON-serialized result as a String.
pub fn execute_wasm_with_vars(
    wasm_bytes: &[u8],
    input_json: Option<&str>,
    data_json: Option<&str>,
) -> Result<String, anyhow::Error> {
    // Create a Wasmtime engine and store
    let engine = Engine::default();
    let mut store = Store::new(&engine, ());

    // Load and compile the WASM module from bytes
    let module = Module::from_binary(&engine, wasm_bytes)?;

    // Create an instance
    let instance = Instance::new(&mut store, &module, &[])?;

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
