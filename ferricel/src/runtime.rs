use wasmtime::*;

/// Execute a compiled WASM module and return the validation result as JSON
///
/// Takes the WASM module bytes and executes the `validate` function.
/// Returns the JSON-serialized result as a String.
pub fn execute_wasm(wasm_bytes: &[u8]) -> Result<String, anyhow::Error> {
    // Create a Wasmtime engine and store
    let engine = Engine::default();
    let mut store = Store::new(&engine, ());

    // Load and compile the WASM module from bytes
    let module = Module::from_binary(&engine, wasm_bytes)?;

    // Create an instance
    let instance = Instance::new(&mut store, &module, &[])?;

    // Get the 'validate' function
    let validate = instance
        .get_typed_func::<(i32, i32), i64>(&mut store, "validate")
        .map_err(|e| anyhow::anyhow!("Failed to get 'validate' function: {}", e))?;

    // Call the validate function with dummy arguments (0, 0)
    // In a real scenario, these would be pointers to input and data
    let encoded_result = validate.call(&mut store, (0, 0))?;

    // Decode the pointer and length from the i64 result
    let ptr = (encoded_result & 0xFFFFFFFF) as u32;
    let len = (encoded_result >> 32) as u32;

    // Get the WASM memory
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| anyhow::anyhow!("Failed to get WASM memory"))?;

    // Read the JSON bytes from WASM memory
    let mut json_bytes = vec![0u8; len as usize];
    memory.read(&store, ptr as usize, &mut json_bytes)?;

    // Convert bytes to UTF-8 string
    let json_string = String::from_utf8(json_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON as UTF-8: {}", e))?;

    Ok(json_string)
}
