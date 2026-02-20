use clap::Parser;

mod cli;
mod cmd;
mod compiler;

fn main() -> Result<(), anyhow::Error> {
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Commands::Build { expression, output } => cmd::build::run(&expression, &output),
    }
}
