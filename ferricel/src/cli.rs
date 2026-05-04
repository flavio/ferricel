use std::path::PathBuf;

use clap::{ArgGroup, Parser as ClapParser, Subcommand, ValueEnum};
use ferricel_types::LogLevel;

/// Ferricel - CEL compiler to WebAssembly
#[derive(ClapParser)]
#[command(name = "ferricel")]
#[command(about = "Compile CEL expressions to WebAssembly", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// CLI-friendly wrapper for LogLevel that implements ValueEnum
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogLevelArg {
    /// Debug level (most verbose)
    Debug,
    /// Info level (default)
    Info,
    /// Warning level
    Warn,
    /// Error level (least verbose)
    Error,
}

impl From<LogLevelArg> for LogLevel {
    fn from(arg: LogLevelArg) -> Self {
        match arg {
            LogLevelArg::Debug => LogLevel::Debug,
            LogLevelArg::Info => LogLevel::Info,
            LogLevelArg::Warn => LogLevel::Warn,
            LogLevelArg::Error => LogLevel::Error,
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Build a CEL expression into a WebAssembly module
    #[command(group = ArgGroup::new("cel_source")
        .required(true)
        .args(&["expression", "expression_file"]))]
    Build {
        /// CEL expression to compile (mutually exclusive with --expression-file)
        #[arg(short, long, conflicts_with = "expression_file")]
        expression: Option<String>,

        /// Path to file containing CEL expression (mutually exclusive with --expression)
        #[arg(long, conflicts_with = "expression")]
        expression_file: Option<PathBuf>,

        /// Output file path
        #[arg(short, long, default_value = "final_cel_program.wasm")]
        output: PathBuf,

        /// Path to protocol buffer descriptor file(s) (can be specified multiple times)
        /// Use protoc --descriptor_set_out to generate these files
        #[arg(long = "proto-descriptor")]
        proto_descriptors: Vec<PathBuf>,

        /// Container (namespace) for type name resolution
        /// Example: "google.protobuf" allows using "Timestamp" instead of "google.protobuf.Timestamp"
        #[arg(long)]
        container: Option<String>,

        /// Declare host extension functions (can be specified multiple times).
        /// Mutually exclusive with --extensions-file.
        ///
        /// Format: [namespace.]function:style:arity
        ///
        ///   namespace   Optional dot-separated namespace prefix. Everything before
        ///               the last dot is the namespace; the last segment is the
        ///               function name.
        ///
        ///   style       One of:
        ///                 global   - callable as func(args) or ns.func(args)
        ///                 receiver - callable as value.func(extra_args)
        ///                            (receiver is always args[0])
        ///                 both     - supports both calling conventions
        ///
        ///   arity       Total number of arguments the host receives, including
        ///               the receiver for receiver-style calls.
        ///
        /// Examples:
        ///   --extensions abs:global:1
        ///       Adds a global function abs(x) with 1 argument.
        ///
        ///   --extensions math.sqrt:global:1
        ///       Adds math.sqrt(x) — namespace "math", function "sqrt", 1 arg.
        ///
        ///   --extensions math.pow:global:2
        ///       Adds math.pow(base, exp) — 2 args.
        ///
        ///   --extensions reverse:receiver:1
        ///       Adds x.reverse() — receiver-style, receiver counts as the 1 arg.
        ///
        ///   --extensions greet:both:2
        ///       Adds greet(name, lang) and name.greet(lang) — both styles, 2 args.
        ///
        /// Note: the host is responsible for providing implementations at evaluation
        /// time. Extensions declared here but not implemented by the host will produce
        /// a runtime error when the expression is evaluated.
        #[arg(
            long = "extensions",
            conflicts_with = "extensions_file",
            value_name = "SPEC"
        )]
        extensions: Vec<String>,

        /// Path to a JSON file declaring host extension functions.
        /// Mutually exclusive with --extensions.
        ///
        /// The file must contain a JSON array of extension declaration objects.
        /// Each object has the following fields:
        ///
        ///   namespace      (string | null)  Optional namespace prefix,
        ///                                   e.g. "math" for math.abs().
        ///   function       (string)         Function name, e.g. "abs".
        ///   global_style   (bool)           True if callable as func(args)
        ///                                   or ns.func(args).
        ///   receiver_style (bool)           True if callable as value.func(args).
        ///                                   Receiver is always args[0].
        ///   num_args       (number)         Total argument count including
        ///                                   receiver for receiver-style calls.
        ///
        /// Example file contents:
        ///   [
        ///     { "namespace": "math", "function": "sqrt",
        ///       "global_style": true, "receiver_style": false, "num_args": 1 },
        ///     { "namespace": null,   "function": "reverse",
        ///       "global_style": false, "receiver_style": true, "num_args": 1 },
        ///     { "namespace": null,   "function": "greet",
        ///       "global_style": true, "receiver_style": true, "num_args": 2 }
        ///   ]
        ///
        /// Note: the host is responsible for providing implementations at evaluation
        /// time. Extensions declared here but not implemented by the host will produce
        /// a runtime error when the expression is evaluated.
        #[arg(
            long = "extensions-file",
            conflicts_with = "extensions",
            value_name = "PATH"
        )]
        extensions_file: Option<PathBuf>,
    },
    /// Run a compiled WebAssembly module
    Run {
        /// Path to the Wasm file to execute
        wasm: PathBuf,

        /// Bindings JSON string containing variable values (mutually exclusive with --bindings-file)
        /// Example: --bindings-json '{"x": 42, "name": "Alice"}'
        #[arg(long, conflicts_with = "bindings_file")]
        bindings_json: Option<String>,

        /// Path to bindings JSON file containing variable values (mutually exclusive with --bindings-json)
        #[arg(long, conflicts_with = "bindings_json")]
        bindings_file: Option<PathBuf>,

        /// Minimum log level for runtime logging
        #[arg(short = 'l', long, value_enum, default_value = "info")]
        log_level: LogLevelArg,
    },
}
