# Introduction

Ferricel compiles [CEL (Common Expression Language)](https://cel.dev)
expressions into WebAssembly modules.

The produced `.wasm` files can then be executed in any Wasm runtime.

## Components

Ferricel provides the following components:

- `ferricel`: a CLI tool that can be used to either compile or run a `.wasm`
  module produced by it
- `ferricel-core`: the pure Rust crate used by `ferricel` CLI. It can be used
  to embed a compiler or a runtime inside of your Rust program

The next sections cover in depth how to handle compilation of CEL expressions
and how to run them.

## Spec

The ["Wasm Spec"](./wasm-spec.md) section illustrates the low level details of the WebAssembly
modules produced by the ferricel compiler.

## Compilation

The `ferricel` CLI tool can be used to build a CEL program to WebAssembly.
You can find more details [here](./compile-cel-to-wasm.md).