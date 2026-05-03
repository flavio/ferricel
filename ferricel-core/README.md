# ferricel-core

Core compiler and runtime library for compiling Common Expression Language (CEL) expressions to WebAssembly.

## Overview

`ferricel-core` provides the core functionality for:
- Compiling CEL expressions into standalone WebAssembly modules
- Executing Wasm modules with variable bindings
- Type handling for integers, unsigned integers, doubles, strings, booleans, lists, and maps

This crate is the reusable library that powers the `ferricel` CLI tool and can be integrated into other Rust projects.

## Features

- **Zero-dependency Wasm modules**: Generated Wasm files are self-contained and portable
- **Variable binding support**: Pass JSON input and data to CEL expressions
- **Type coercion**: Automatic conversion between numeric types following CEL spec
- **Comprehensive operators**: Arithmetic, comparison, logical, string, and list operations
- **Logging support**: Configurable logging during compilation and execution

## Usage

### Basic Compilation and Execution

```rust
use ferricel_core::{compile_cel_to_wasm};
use ferricel_core::runtime;
use ferricel_types::LogLevel;
use slog::Logger;

// Compile a CEL expression to Wasm
let wasm_bytes = compile_cel_to_wasm("1 + 1")?;

// Execute the Wasm module
let result = runtime::execute_wasm(
    &wasm_bytes,
    None,  // bindings JSON (map of variable names to values)
    LogLevel::Error,
    logger
)?;

println!("Result: {}", result); // {"result": 2}
```

### With Variable Bindings

```rust
use ferricel_core::evaluate_cel;
use ferricel_types::LogLevel;

// Use the convenience function that compiles and executes in one step
let result = evaluate_cel(
    "input.x + input.y",
    Some(r#"{"x": 10, "y": 20}"#),
    None,
    LogLevel::Error,
    logger
)?;

println!("Result: {}", result); // 30
```

### Granular API

```rust
use ferricel_core::compiler;
use ferricel_core::runtime;

// Use the granular API for more control
let wasm = compiler::compile_cel_to_wasm("2 * 3")?;
let result = runtime::execute_wasm(&wasm, None, log_level, logger)?;
```

## Architecture

- **`compiler` module**: Parses CEL expressions using the `cel` crate, walks the AST, and generates WebAssembly bytecode using `walrus`
- **`runtime` module**: Executes Wasm modules using `wasmtime`, providing variable injection and result extraction

## Testing

Unit tests are located in `tests/compiler_tests.rs` and cover:
- Arithmetic operations (int, uint, double)
- Comparison operators
- Logical operators
- String operations
- List operations
- Type conversions
- Variable bindings

Run tests with:
```bash
cargo test --package ferricel-core
```

## CEL Specification

This implementation follows the [Common Expression Language specification](https://github.com/google/cel-spec). See the conformance tests in the `conformance` crate for detailed compliance information.

## License

See the root workspace LICENSE file.
