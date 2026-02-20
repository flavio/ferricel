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
}
