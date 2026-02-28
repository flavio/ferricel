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
        } => cmd::build::run(expression, expression_file, &output),
        cli::Commands::Run {
            wasm,
            input_json,
            input_file,
            data_json,
            data_file,
            log_level,
        } => cmd::run::run(
            &wasm, input_json, input_file, data_json, data_file, log_level,
        ),
    }
}
