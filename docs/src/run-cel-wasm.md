# Running a CEL program compiled to Wasm

There are two ways to run a CEL program compiled to Wasm:

- **`ferricel run` CLI** — simple command-line execution; does not support host extensions.
- **`ferricel-core` Rust crate** — more flexible; supports host extensions and custom configurations.

## Using the `ferricel` CLI

Here's a simple CEL program. Save it to a file named `validate-balance.cel`:


```cel
account.balance >= transaction.withdrawal
    || (account.overdraftProtection
    && account.overdraftLimit >= transaction.withdrawal  - account.balance)
```

Compile it to a Wasm module:

```shell
ferricel build --expression-file validate-balance.cel -o validate-balance.wasm
```

Create a bindings file named `validate-balance.bindings.json` with the input data:

```json
{
  "account": {
    "balance": 500,
    "overdraftProtection": true,
    "overdraftLimit": 1000
  },
  "transaction": {
    "withdrawal": 700
  }
}
```

Run the compiled Wasm module with the bindings:

```shell
ferricel run \
    --bindings-file validate-balance.bindings.json \
    validate-balance.wasm
```

Output:

```shell
true
```

Alternatively, pass bindings as an inline JSON string:

```shell
ferricel run \
    --bindings-json '{"account":{"balance":500,"overdraftProtection":true,"overdraftLimit":0},"transaction":{"withdrawal":700}}' \
    validate-balance.wasm 
```

Output:

```shell
false
```

## Using the `ferricel-core` Rust crate

The `ferricel-core` crate provides programmatic control over compilation and
evaluation. This is useful when you need to evaluate many expressions, integrate
CEL into a larger system, or use host extensions.

### Basic evaluation

Compile a CEL expression and evaluate it with bindings:

```rust
use ferricel_core::{compiler, runtime};

let wasm = compiler::Builder::new()
    .build()
    .compile("x * 2 + 1")?;

let result = runtime::Builder::new()
    .with_wasm(wasm)
    .build()?
    .eval(Some(r#"{"x": 10}"#))?;

assert_eq!(result, "21");
```

The `eval` method accepts JSON-encoded variable bindings and returns a
JSON-encoded result string.

### Host extensions

Register host-provided functions that the CEL expression can call:

```rust
use ferricel_core::{compiler, runtime};
use ferricel_types::extensions::ExtensionDecl;

// Declare the extension at compile time
let abs_decl = ExtensionDecl {
    namespace: None,
    function: "abs".to_string(),
    receiver_style: false,
    global_style: true,
    num_args: 1,
};

// Compile the CEL expression with the extension
let wasm = compiler::Builder::new()
    .with_extension(abs_decl.clone())
    .build()
    .compile("abs(x)")?;

// Register the implementation at runtime
let result = runtime::Builder::new()
    .with_extension(abs_decl, |args| {
        let n = args[0].as_i64().unwrap_or(0);
        Ok(serde_json::Value::Number(n.abs().into()))
    })
    .with_wasm(wasm)
    .build()?
    .eval(Some(r#"{"x": -42}"#))?;

assert_eq!(result, "42");
```

The extension declaration specifies the function signature at compile time (for
validation), and the implementation is provided at runtime. The host is
responsible for marshalling JSON values to and from the extension function.

### Logging

Configure logging during evaluation with a custom logger and log level:

```rust
use ferricel_core::runtime;
use ferricel_types::LogLevel;
use slog::{Drain, Logger, o};

let decorator = slog_term::PlainSyncDecorator::new(std::io::stderr());
let drain = slog_term::FullFormat::new(decorator).build().fuse();
let logger = Logger::root(drain, o!());

let result = runtime::Builder::new()
    .with_logger(logger)
    .with_log_level(LogLevel::Debug)
    .with_wasm(wasm)
    .build()?
    .eval(Some(r#"{"x": 10}"#))?;
```

### Custom Wasmtime configuration

For advanced use cases, provide your own `wasmtime::Engine` with custom settings
(e.g. fuel metering, epoch interruption):

```rust
use ferricel_core::{compiler, runtime};
use wasmtime::{Config, Engine as WasmEngine};

let mut config = Config::new();
config.consume_fuel(true);

let wasm_engine = WasmEngine::new(&config)?;
let wasm = compiler::Builder::new().build().compile("1 + 1")?;

let result = runtime::Builder::new()
    .with_engine(wasm_engine)
    .with_wasm(wasm)
    .build()?
    .eval(None)?;
```

### Protobuf bindings

For type-safe variable bindings that preserve full fidelity (bytes, uint,
timestamp, duration, etc.), use `eval_proto` with protobuf-encoded bindings:

```rust,ignore
let result = engine.eval_proto(&bindings_proto_bytes)?;
```

This avoids the JSON round-trip and preserves exact type information.

### Memory management and performance

> [!IMPORTANT]
> CEL programs compiled to Wasm use a leaking allocator: memory is never
> deallocated during evaluation. This is not an issue in practice because CEL
> expressions are short-lived.

However, the consequence is that **each evaluation starts with a fresh Wasm
instance**. This ensures no memory carries over between calls and prevents
unbounded memory growth.

To optimize performance despite this, the `Builder` uses `wasmtime::InstancePre`
to pre-link the module at build time. This amortizes compilation cost:

- `Builder::build()` parses the Wasm bytes once and pre-links all host functions.
- Each `eval()` call only pays the cost of instantiation.

For workloads with many evaluations, creating a single `Engine` and reusing it
across multiple `eval()` calls is the recommended pattern.

