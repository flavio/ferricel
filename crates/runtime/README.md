The Wasm guest runtime for [ferricel](../../README.md), a CEL-to-WebAssembly compiler.

## What this crate does

This crate is compiled to `wasm32-unknown-unknown` and its binary is **embedded directly into every `.wasm` file** that `ferricel-core` produces.
Each output Wasm module is fully self-contained — there are no external runtime dependencies at evaluation time.

When `ferricel-core` compiles a CEL expression, the emitted Wasm instructions call into this library for every operator, built-in function, and type conversion.
This crate is the execution engine that gives those calls meaning.
