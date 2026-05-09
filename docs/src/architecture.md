# Compiler Architecture

Ferricel compiles [CEL](https://cel.dev) expressions into self-contained WebAssembly modules.
The process involves two distinct Rust artifacts: the **runtime** and the **compiler**.

### The Runtime

The `runtime` crate is a Rust library compiled to `wasm32-unknown-unknown`.
It provides the low-level functions that a compiled CEL program calls at
execution time, like memory allocation, value serialization, arithmetic helpers,
string operations, and so on.
Because the target is bare Wasm (no WASI, no OS), it is entirely self-contained.


### The Compiler

`ferricel-core` contains the compiler. It parses the CEL source using the
parser that is part of the [`cel`](https://crates.io/crates/cel) crate.
The parser produces a typed AST, which the compiler then traverses, emitting
WebAssembly instructions.
The WebAssembly instructions are produced using the [`walrus`](https://crates.io/crates/walrus)
crate.

Rather than generating a Wasm module from scratch, the compiler loads the
pre-embedded `runtime.wasm` module and injects the compiled CEL program into it.
A dead-code-elimination pass (based on [`walrus::passes::gc`](https://docs.rs/walrus/latest/walrus/passes/gc/index.html))
then removes any runtime functions the program does not call, keeping output
files small.

The result is a single `.wasm` file that is fully self-contained and can be
executed anywhere a WebAssembly runtime is available.