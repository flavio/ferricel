use std::{
    fs,
    path::{Path, PathBuf},
};

use ferricel_core::runtime;
use slog::{Drain, Logger, o};

use crate::cli::LogLevelArg;

pub fn run(
    wasm_path: &Path,
    bindings_json: Option<String>,
    bindings_file: Option<PathBuf>,
    log_level: LogLevelArg,
) -> Result<(), anyhow::Error> {
    // Check if Wasm file exists
    if !wasm_path.exists() {
        anyhow::bail!("Wasm file not found at {}", wasm_path.display());
    }

    println!("Loading Wasm module: {}", wasm_path.display());

    // Read the Wasm file
    let wasm_bytes = fs::read(wasm_path)?;

    // Resolve bindings: either from --bindings-json or --bindings-file
    let bindings = if let Some(json) = bindings_json {
        Some(json)
    } else if let Some(path) = bindings_file {
        if !path.exists() {
            anyhow::bail!("Bindings file not found at {}", path.display());
        }
        println!("Loading bindings from file: {}", path.display());
        Some(fs::read_to_string(path)?)
    } else {
        None
    };

    // Create simple logger with PlainSyncDecorator (no Mutex needed with FullFormat)
    let decorator = slog_term::PlainSyncDecorator::new(std::io::stderr());
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let logger = Logger::root(drain, o!());

    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_log_level(log_level.into())
        .with_wasm(wasm_bytes)
        .build()?
        .eval(bindings.as_deref())?;

    println!("Execution result: {}", result);
    Ok(())
}
