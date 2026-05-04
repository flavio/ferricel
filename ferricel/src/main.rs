use clap::Parser;

mod cli;
mod cmd;

fn main() -> Result<(), anyhow::Error> {
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Commands::Build {
            expression,
            expression_file,
            output,
            proto_descriptors,
            container,
            extensions,
            extensions_file,
        } => cmd::build::run(
            expression,
            expression_file,
            &output,
            proto_descriptors,
            container,
            extensions,
            extensions_file,
        ),
        cli::Commands::Run {
            wasm,
            bindings_json,
            bindings_file,
            log_level,
        } => cmd::run::run(&wasm, bindings_json, bindings_file, log_level),
    }
}
