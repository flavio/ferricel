use std::fs;
use std::path::Path;

use crate::runtime;

pub fn run(wasm_path: &Path) -> Result<(), anyhow::Error> {
    // Check if WASM file exists
    if !wasm_path.exists() {
        anyhow::bail!("WASM file not found at {}", wasm_path.display());
    }

    println!("Loading WASM module: {}", wasm_path.display());

    // Read the WASM file
    let wasm_bytes = fs::read(wasm_path)?;

    // Execute the WASM module
    let result = runtime::execute_wasm(&wasm_bytes)?;

    println!("Execution result: {}", result);
    Ok(())
}
