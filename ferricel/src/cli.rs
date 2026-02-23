use std::path::PathBuf;

use clap::{Parser as ClapParser, Subcommand};

/// Ferricel - CEL compiler to WebAssembly
#[derive(ClapParser)]
#[command(name = "ferricel")]
#[command(about = "Compile CEL expressions to WebAssembly", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Build a CEL expression into a WebAssembly module
    Build {
        /// CEL expression to compile
        #[arg(short, long)]
        expression: String,

        /// Output file path
        #[arg(short, long, default_value = "final_cel_program.wasm")]
        output: PathBuf,
    },
    /// Run a compiled WebAssembly module
    Run {
        /// Path to the WASM file to execute
        wasm: PathBuf,

        /// Input JSON string (mutually exclusive with --input-file)
        #[arg(long, conflicts_with = "input_file")]
        input_json: Option<String>,

        /// Path to input JSON file (mutually exclusive with --input-json)
        #[arg(long, conflicts_with = "input_json")]
        input_file: Option<PathBuf>,

        /// Data JSON string (mutually exclusive with --data-file)
        #[arg(long, conflicts_with = "data_file")]
        data_json: Option<String>,

        /// Path to data JSON file (mutually exclusive with --data-json)
        #[arg(long, conflicts_with = "data_json")]
        data_file: Option<PathBuf>,
    },
}
