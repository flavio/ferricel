[![Docs](https://img.shields.io/badge/docs-latest-blue)](https://flavio.github.io/ferricel/)

Ferricel compiles [CEL (Common Expression Language)](https://cel.dev)
expressions into WebAssembly modules.

The produced `.wasm` files can then be executed in any Wasm runtime.

> [!NOTE]
> **Development Transparency**: Ferricel has been developed with the aid of a
> code assistant (Claude Sonnet/Opus 4.6).
>
> The code assistant was used for implementation, testing, and documentation.
> All development has been conducted under direct human supervision,
> with code review and validation performed throughout.

## Components

Ferricel provides the following components:

- `ferricel`: a CLI tool for compiling CEL expressions to WebAssembly or running compiled `.wasm` modules
- `ferricel-core`: a pure Rust library that powers `ferricel`. You can use it to embed a compiler or runtime in your Rust program

## Features

Ferricel targets full compliance with the [CEL specification](https://github.com/google/cel-spec)
and the [cel-go extension libraries](https://pkg.go.dev/github.com/google/cel-go/ext).
Conformance is validated against the official CEL conformance test suite.

Ferricel also supports the [Kubernetes CEL validation libraries](https://kubernetes.io/docs/reference/using-api/cel/).

For more details about Ferricel compliance, please refer to the
docs.

Ferricel also supports extending CEL programs with custom functions implemented by the WebAssembly host.
See [Host Extensions](https://flavio.github.io/ferricel/run-cel-wasm.html#host-extensions) for details.

## Usage (CLI)

Let's take this small CEL program:

```cel
account.balance >= transaction.withdrawal
    || (account.overdraftProtection
    && account.overdraftLimit >= transaction.withdrawal  - account.balance)
```

We'll compile it to a WebAssembly module using the `ferricel` CLI:

```shell
ferricel build --expression-file validate-balance.cel -o validate-balance.wasm
```

Now we can run it with the `ferricel run` command. We need to provide the data to be evaluated, which we call "bindings".

Let's create a bindings file named `validate-balance.bindings.json` with
the following contents:

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

We can now run the compiled Wasm module with the bindings:

```shell
ferricel run \
    --bindings-file validate-balance.bindings.json \
    validate-balance.wasm
```

The evaluation produces the following output:

```shell
true
```

Alternatively, we can pass bindings as an inline JSON string:

```shell
ferricel run \
    --bindings-json '{"account":{"balance":500,"overdraftProtection":true,"overdraftLimit":0},"transaction":{"withdrawal":700}}' \
    validate-balance.wasm
```

In this case, the program runs with `overdraftLimit` set to `0`, which causes the validation to return a different result:

```shell
false
```

## Usage (Rust)

You can use `ferricel-core` to compile CEL expressions and execute them from Rust.

See the [ferricel-core documentation](https://flavio.github.io/ferricel/) for examples and API details.

## Installation

Download pre-built binaries from [GitHub Releases](https://github.com/flavio/ferricel/releases).

Or install from source using Cargo:

```sh
cargo install ferricel
```

> [!NOTE]
> This installation method requires the `protoc` binary to be available on your system.

## Name

The name "Ferricel" is a pun combining "ferrous" (relating to iron and Rust,
the programming language) and "CEL" (Common Expression Language).
