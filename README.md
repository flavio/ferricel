Ferricel compiles [CEL (Common Expression Language)](https://cel.dev)
expressions into WebAssembly modules.

The produced `.wasm` files can then be executed in any Wasm runtime.

## Components

Ferricel provides the following components:

- `ferricel`: a CLI tool that can be used to either compile or run a `.wasm`
  module produced by it
- `ferricel-core`: the pure Rust crate used by `ferricel` CLI. It can be used
  to embed a compiler or a runtime inside of your Rust program