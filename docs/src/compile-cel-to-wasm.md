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
by the host at evaluation time. Without a declaration, the compiler treats any
unknown function call as a `no matching overload` error baked statically into
the Wasm module. Declaring an extension tells the compiler to emit a real host
call instead, dispatched at runtime via `cel_call_extension`.

Extensions can be declared inline with `--extensions` (repeatable):

```sh
ferricel build \
  --expression 'math.sqrt(x)' \
  --extensions math.sqrt:global:1 \
  --output result.wasm
```

Or in a JSON file with `--extensions-file`:

```sh
ferricel build \
  --expression 'math.sqrt(x)' \
  --extensions-file extensions.json \
  --output result.wasm
```

The two flags are mutually exclusive.

### `--extensions` format

Each `--extensions` value follows the pattern `[namespace.]function:style:arity`:

| Segment     | Description |
|-------------|-------------|
| `namespace` | Optional dot-separated namespace prefix (e.g. `math` in `math.sqrt`). |
| `function`  | Function name (the last dot-separated segment). |
| `style`     | One of `global`, `receiver`, or `both` (see below). |
| `arity`     | Total number of arguments the host receives, including the receiver for receiver-style calls. |

**Calling styles:**

| Style      | Invocation form |
|------------|-----------------|
| `global`   | `func(args)` or `ns.func(args)` |
| `receiver` | `value.func(extra_args)` — receiver is always `args[0]` |
| `both`     | Both of the above |

**Examples:**

```sh
--extensions abs:global:1            # abs(x) — 1 arg
--extensions math.sqrt:global:1      # math.sqrt(x) — namespace "math", 1 arg
--extensions math.pow:global:2       # math.pow(base, exp) — 2 args
--extensions reverse:receiver:1      # x.reverse() — receiver counts as the 1 arg
--extensions greet:both:2            # greet(name, lang) and name.greet(lang)
```

### `--extensions-file` format

The file must contain a JSON array of extension declaration objects:

```json
[
  { "namespace": "math", "function": "sqrt",
    "global_style": true, "receiver_style": false, "num_args": 1 },
  { "namespace": null, "function": "reverse",
    "global_style": false, "receiver_style": true, "num_args": 1 },
  { "namespace": null, "function": "greet",
    "global_style": true, "receiver_style": true, "num_args": 2 }
]
```

> **Note:** the host is responsible for providing implementations at evaluation
> time. Extensions declared at compile time but not implemented by the host will
> produce a runtime error when the expression is evaluated.

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

