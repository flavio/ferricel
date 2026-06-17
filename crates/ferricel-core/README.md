# ferricel-core

[![Docs](https://img.shields.io/badge/docs-latest-blue)](https://flavio.github.io/ferricel/)

`ferricel-core` is the Rust library that powers [`ferricel`](https://crates.io/crates/ferricel).
It provides a compiler and runtime for [CEL (Common Expression Language)](https://cel.dev),
targeting WebAssembly as the compilation output.

Use it to embed CEL compilation and evaluation directly in your Rust program.

## Usage

### Basic evaluation

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

### Host extensions

Register host-provided functions that a CEL expression can call at runtime:

```rust
use ferricel_core::{compiler, runtime};
use ferricel_types::extensions::ExtensionDecl;

let abs_decl = ExtensionDecl {
    namespace: None,
    function: "abs".to_string(),
    receiver_style: false,
    global_style: true,
    num_args: 1,
};

let wasm = compiler::Builder::new()
    .with_extension(abs_decl.clone())
    .build()
    .compile("abs(x)")?;

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

For more examples (logging, custom Wasmtime config, protobuf bindings, performance tips),
see the [user guide](https://flavio.github.io/ferricel/run-cel-wasm.html).

## Related crates

- [`ferricel`](https://crates.io/crates/ferricel) — CLI tool built on top of this library
- [`ferricel-types`](https://crates.io/crates/ferricel-types) — shared type definitions
