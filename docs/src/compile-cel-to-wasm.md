# Compiling a CEL Program to Wasm

The `ferricel build` command compiles a CEL expression into a self-contained
WebAssembly module.

The compiler removes all unused runtime functions from the final Wasm module.
Beyond that, it performs no additional optimizations. For smaller or faster
binaries, you can post-process the output with [`wasm-opt`](https://github.com/WebAssembly/binaryen).

## Basic Usage

Pass a CEL expression directly with `--expression`:

```sh
ferricel build --expression '1 + 1' --output result.wasm
```

Or provide the expression in a file with `--expression-file`:

```sh
ferricel build --expression-file my_program.cel --output result.wasm
```

The two flags are mutually exclusive. If `--output` is omitted, the module is
written to `final_cel_program.wasm` in the current directory.

## Host Extensions

Host extensions allow the compiled CEL expression to call functions implemented
by the host at evaluation time. See the [Host Extensions](./host-extensions.md)
chapter for full documentation, including CLI flags, the Rust API, and builder
chains.

## Defining a CEL `container`

A CEL container acts as a namespace for type name resolution. When a container
is set, unqualified type names are resolved relative to it first.

For example, with `--container google.protobuf` you can write `Timestamp`
instead of `google.protobuf.Timestamp` in your CEL expression:

```sh
ferricel build \
  --expression 'Timestamp{seconds: 0}' \
  --container google.protobuf \
  --proto-descriptor descriptor.pb \
  --output result.wasm
```

## Using Proto

To use Protocol Buffer message types in a CEL expression, provide a compiled
descriptor set with `--proto-descriptor`. The flag can be repeated to merge
multiple descriptor files:

```sh
ferricel build \
  --expression 'req.user_id != ""' \
  --proto-descriptor api.pb \
  --output result.wasm
```

Generate a descriptor set from your `.proto` files with `protoc`:

```sh
protoc --descriptor_set_out=api.pb --include_imports api.proto
```

Multiple descriptor files are merged automatically:

```sh
ferricel build \
  --expression 'a.field == b.field' \
  --proto-descriptor types_a.pb \
  --proto-descriptor types_b.pb \
  --output result.wasm
```

