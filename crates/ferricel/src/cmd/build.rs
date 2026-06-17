use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use ferricel_core::compiler::Builder;
use prost::Message;
use prost_types::FileDescriptorSet;
use slog::{Drain, Logger, o};

use crate::cmd::extensions::resolve_extensions;

pub fn run(
    expression: Option<String>,
    expression_file: Option<PathBuf>,
    output_path: &Path,
    proto_descriptors: Vec<PathBuf>,
    container: Option<String>,
    extensions_specs: Vec<String>,
    extensions_file: Option<PathBuf>,
) -> Result<(), anyhow::Error> {
    // Determine CEL source - clap ensures exactly one is Some
    let cel_code = match (expression, expression_file) {
        (Some(expr), None) => expr,
        (None, Some(path)) => fs::read_to_string(&path)
            .with_context(|| format!("Failed to read CEL file: {}", path.display()))?,
        _ => unreachable!("Clap should enforce mutual exclusivity and require one source"),
    };

    // Read and merge proto descriptor files if provided
    let merged_descriptor = if proto_descriptors.is_empty() {
        None
    } else {
        let mut merged = FileDescriptorSet { file: vec![] };

        for descriptor_path in &proto_descriptors {
            let descriptor_bytes = fs::read(descriptor_path).with_context(|| {
                format!(
                    "Failed to read proto descriptor file: {}",
                    descriptor_path.display()
                )
            })?;

            // Parse the FileDescriptorSet
            let fds = FileDescriptorSet::decode(&descriptor_bytes[..]).with_context(|| {
                format!(
                    "Failed to parse proto descriptor file: {}",
                    descriptor_path.display()
                )
            })?;

            // Merge all files into the combined descriptor set
            merged.file.extend(fds.file);
        }

        // Serialize back to bytes
        let mut buffer = Vec::new();
        merged
            .encode(&mut buffer)
            .context("Failed to encode merged descriptor set")?;
        Some(buffer)
    };

    // Resolve extension declarations from --extensions specs or --extensions-file
    let extension_decls = resolve_extensions(extensions_specs, extensions_file.as_deref())?;

    // Create a logger to stderr for compilation warnings
    let decorator = slog_term::PlainSyncDecorator::new(std::io::stderr());
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let logger = Logger::root(drain, o!());

    // Build the compiler using the builder API
    let mut builder = Builder::new().with_logger(logger);
    if let Some(bytes) = merged_descriptor {
        builder = builder.with_proto_descriptor(bytes)?;
    }
    if let Some(c) = container {
        builder = builder.with_container(c);
    }
    for decl in extension_decls {
        builder = builder.with_extension(decl);
    }
    let compiler = builder.build();

    let wasm_bytes = compiler.compile(&cel_code)?;

    // Write the Wasm bytes to the output file
    fs::write(output_path, wasm_bytes)?;

    println!("Successfully compiled CEL into: {}", output_path.display());
    Ok(())
}
