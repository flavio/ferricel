# ferricel

[![Docs](https://img.shields.io/badge/docs-latest-blue)](https://flavio.github.io/ferricel/)

`ferricel` is a CLI tool that compiles [CEL (Common Expression Language)](https://cel.dev)
expressions into self-contained WebAssembly modules, and runs them.

The produced `.wasm` files can be executed in any Wasm runtime.

## Installation

Download pre-built binaries from [GitHub Releases](https://github.com/flavio/ferricel/releases),
or install from source:

```sh
cargo install ferricel
```

## Usage

Compile a CEL expression to a Wasm module:

```sh
ferricel build --expression-file validate-balance.cel -o validate-balance.wasm
```

Run the compiled module with JSON bindings:

```sh
ferricel run --bindings-file validate-balance.bindings.json validate-balance.wasm
```

Or pass bindings inline:

```sh
ferricel run --bindings-json '{"x": 42}' result.wasm
```

For full documentation including host extensions, proto support, and more, see
the [user guide](https://flavio.github.io/ferricel/).

## Related crates

- [`ferricel-core`](https://crates.io/crates/ferricel-core) — library for embedding the compiler and runtime in Rust programs
- [`ferricel-types`](https://crates.io/crates/ferricel-types) — shared type definitions
