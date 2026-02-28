use std::fs;
use std::path::{Path, PathBuf};

use ferricel_core::runtime;
use slog::{Drain, Logger, o};

use crate::cli::LogLevelArg;

pub fn run(
    wasm_path: &Path,
    input_json: Option<String>,
    input_file: Option<PathBuf>,
    data_json: Option<String>,
    data_file: Option<PathBuf>,
    log_level: LogLevelArg,
) -> Result<(), anyhow::Error> {
    // Check if WASM file exists
    if !wasm_path.exists() {
        anyhow::bail!("WASM file not found at {}", wasm_path.display());
    }

    println!("Loading WASM module: {}", wasm_path.display());

    // Read the WASM file
    let wasm_bytes = fs::read(wasm_path)?;

    // Resolve input: either from --input-json or --input-file
    let input = if let Some(json) = input_json {
        Some(json)
    } else if let Some(path) = input_file {
        if !path.exists() {
            anyhow::bail!("Input file not found at {}", path.display());
        }
        println!("Loading input from file: {}", path.display());
        Some(fs::read_to_string(path)?)
    } else {
        None
    };

    // Resolve data: either from --data-json or --data-file
    let data = if let Some(json) = data_json {
        Some(json)
    } else if let Some(path) = data_file {
        if !path.exists() {
            anyhow::bail!("Data file not found at {}", path.display());
        }
        println!("Loading data from file: {}", path.display());
        Some(fs::read_to_string(path)?)
    } else {
        None
    };

    // Execute the WASM module with variables
    let log_level = log_level.into();

    // Create simple logger with PlainSyncDecorator (no Mutex needed with FullFormat)
    let decorator = slog_term::PlainSyncDecorator::new(std::io::stderr());
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let logger = Logger::root(drain, o!());

    let result = runtime::execute_wasm_with_vars(
        &wasm_bytes,
        input.as_deref(),
        data.as_deref(),
        log_level,
        logger,
    )?;

    println!("Execution result: {}", result);
    Ok(())
}
