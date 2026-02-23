use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::compiler;

pub fn run(
    expression: Option<String>,
    expression_file: Option<PathBuf>,
    output_path: &Path,
) -> Result<(), anyhow::Error> {
    // Determine CEL source - clap ensures exactly one is Some
    let cel_code = match (expression, expression_file) {
        (Some(expr), None) => expr,
        (None, Some(path)) => fs::read_to_string(&path)
            .with_context(|| format!("Failed to read CEL file: {}", path.display()))?,
        _ => unreachable!("Clap should enforce mutual exclusivity and require one source"),
    };

    // Compile the CEL expression to WASM bytes
    let wasm_bytes = compiler::compile_cel_to_wasm(&cel_code)?;

    // Write the WASM bytes to the output file
    fs::write(output_path, wasm_bytes)?;

    println!("Successfully compiled CEL into: {}", output_path.display());
    Ok(())
}
