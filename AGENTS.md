# Agent Development Guide

A file for [guiding coding agents](https://agents.md/).

## Commands

- **Build:** `make ferricel`
- **Test (unit):** `make unit-tests`
- **Test (end to end):** `make e2e-tests`
- **Formatting**: `make fmt`
- **Linting**: `make lint`
- **Linting (fix some issues automatically)**: `make lint-fix`

## Components

- runtime: provide functions that the CEL program will use at runtime. This is compiled
  to wasm32-unknown-unknown and embedded into the final WASM module.
- ferricel: CLI program with two major subcommand:
  - `build`: read a CEL program, walks the AST and generate the final WASM module
  - `run`: load a WASM module and invoke its `validate` function. Print the result to STDOUT

## Testing

Favor unit test. Use `rstest` when possible to avoid repetition.

There are end-to-end tests for the `ferricel` CLI program under the `ferricel/tests` folder. Writing
new end-to-end tests should be done only when the CLI subcommand/flags are changed. They are expensive
to run, unit tests are to be preferred.

## Development Principles

The CEL official specification must absolutely be respected. It's the north start when doing changes or
implementing new features inside of the compiler.

After new changes are done, the `unit-tests` must be passing.

The `runtime` is always going to be embedded into the final WASM file. The API can be changed at any time,
there are no backward compatibility concerns. Each WASM file produced by the tool is "self-sustained".
