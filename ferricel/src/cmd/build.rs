use std::fs;
use std::path::Path;

use crate::compiler;

pub fn run(cel_code: &str, output_path: &Path) -> Result<(), anyhow::Error> {
    // Compile the CEL expression to WASM bytes
    let wasm_bytes = compiler::compile_cel_to_wasm(cel_code)?;

    // Write the WASM bytes to the output file
    fs::write(output_path, wasm_bytes)?;

    println!("Successfully compiled CEL into: {}", output_path.display());
    Ok(())
}
