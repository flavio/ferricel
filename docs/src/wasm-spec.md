# Wasm Spec

This chapter provides the low-level details about the WebAssembly module
produced by the ferricel compiler.

This information is useful if you plan to load the `.wasm` modules directly
without using the helpers provided by the `ferricel-core` crate.

## Data Exchange

The host and the Wasm guest exchange data using the JSON format.

The host reads and writes data to the WebAssembly linear memory.

Several functions pass or return memory regions through a single `i64` value.
The rest of this document refers to this value as "packed pointer".

The encoding packs a pointer and a byte length into the two 32-bit halves:

```text
bits  0–31  → pointer (offset into Wasm linear memory)
bits 32–63  → byte length
```

In pseudo-code:

```text
packed = (len as i64) << 32 | (ptr as i64)
ptr    = (packed & 0xFFFF_FFFF) as u32
len    = (packed >> 32) as u32
```

## Memory Management

> [!IMPORTANT]
> The code produced by the compiler uses a leaking allocator: memory is never
> released during evaluation.

Given the short lifetime of a CEL program, this is not an issue in practice.

Do not reuse an instantiated WASM module for multiple evaluations—its memory
usage will keep growing. Instead, instantiate a new WASM module for each
evaluation.

## Exported Functions

### `cel_malloc(len: usize) -> *mut u8`

Allocates `len` bytes in Wasm linear memory and returns a pointer to the
allocated buffer.

| Parameter | Type    | Description |
|-----------|---------|-------------|
| `len`     | `usize` | Number of bytes to allocate. |

**Returns** a pointer to the newly allocated buffer within Wasm linear memory.

The allocator is a bump-pointer arena: `dealloc` is a no-op and memory is
released only when the host drops the Wasm instance. Hosts must call this
function to obtain a valid buffer before writing input data (e.g. bindings)
into Wasm memory.

Use this function to load into the instantiate Wasm module the data to be
processed.

---

### `cel_set_log_level(level: i32)`

Sets the minimum log level for messages emitted via `env::cel_log`.

| Parameter | Type  | Description |
|-----------|-------|-------------|
| `level`   | `i32` | Log level threshold (see table below). Values outside the valid range are clamped. |

The log levels are:

| Value | Level   |
|------:|---------|
| `0`   | Debug   |
| `1`   | Info *(default)* |
| `2`   | Warn    |
| `3`   | Error   |

Messages below the configured level are suppressed and `env::cel_log` will not
be called for them.

---

### `evaluate(bindings: i64) -> i64`

Evaluates the compiled CEL expression using JSON-encoded variable bindings.

| Parameter   | Type  | Description |
|-------------|-------|-------------|
| `bindings`  | `i64` | Packed pointer to a UTF-8 JSON object mapping variable names to their values. |

The `bindings` value points to the data previously loaded via `cel_malloc`.

**Returns** a packed `i64` pointing to a UTF-8 JSON string that contains the
result of the CEL expression.

If the expression produces a runtime error (overflow, divide-by-zero, unbound
variable, etc.) the module traps and the host receives an error from the call.

---

### `evaluate_proto(bindings: i64) -> i64`

Evaluates the compiled CEL expression using Protobuf-encoded variable bindings.

| Parameter   | Type  | Description |
|-------------|-------|-------------|
| `bindings`  | `i64` | Packed pointer to a serialized `ferricel.Bindings` protobuf message. |

**Returns** a packed `i64` pointing to a UTF-8 JSON string that contains the
result of the CEL expression (same format as `evaluate`).

Runtime errors cause a trap, identical to `evaluate`.

## Imported Functions

The module imports three functions from the `env` module. All three must be
provided (or stubbed) by the host at instantiation time.

### `env::cel_log(ptr: i32, len: i32)`

Called by the runtime to emit a structured log event.

| Parameter | Type  | Description |
|-----------|-------|-------------|
| `ptr`     | `i32` | Offset in Wasm linear memory of a UTF-8 JSON-encoded `LogEvent` object. |
| `len`     | `i32` | Byte length of the JSON payload. |

The `LogEvent` object is a JSON structure like:

```json
{
  "level": "error",
  "message": "division by zero",
  "file": "main.rs",
  "line": 42,
  "column": 15,
  "extra": { ... }
}
```

The `level` field is one of `"error"`, `"warn"`, `"info"`, or `"debug"`.
The `extra` field is optional and contains arbitrary key-value pairs.

A host that does not need log output can satisfy this import with a no-op
function.

---

### `env::cel_abort(packed: i64)`

Called by the runtime when a fatal runtime error occurs (e.g. divide-by-zero,
integer overflow, unbound variable).

| Parameter | Type  | Description |
|-----------|-------|-------------|
| `packed`  | `i64` | Packed pointer to a UTF-8 error message. |

The host implementation is expected to surface the error message and return an
error to the caller (e.g. by trapping or returning `Err`).

---

### `env::cel_call_extension(request: i64) -> i64`

Invokes a host-provided extension function by name.

| Parameter  | Type  | Description |
|------------|-------|-------------|
| `request`  | `i64` | Packed pointer to a UTF-8 JSON-encoded extension call request. |

The request JSON has the structure:

```json
{
  "namespace": "math",
  "function": "greatest",
  "args": [10, 20, 15]
}
```

Where `namespace` is optional (may be `null`), `function` is the function name, and `args` is an array of JSON-encoded CEL values.

**Returns** a packed `i64` pointing to a UTF-8 JSON string containing the result. The response is a serialized `CelValue`:

```json
{
  "type": "int",
  "value": 20
}
```

The Wasm module calls this import whenever a compiled CEL expression invokes a
function that was registered as a host extension at compile time. The host is
responsible for dispatching the call to the correct implementation based on the
`namespace` and `function` fields in the request. If no extensions are
used, this import can be satisfied with a stub that traps.

> [!NOTE]
> This import is only present if the compiled CEL expression uses host extensions.

## Producers Metadata

Each compiled module includes a standard WebAssembly
[`producers`](https://github.com/WebAssembly/tool-conventions/blob/main/ProducersSection.md)
custom section that records which tools produced it. The section has no
semantic effect on execution and can be safely stripped, but it is useful for
debugging and toolchain analytics.

Ferricel adds the following entries:

| Field          | Name       | Version |
|----------------|------------|---------|
| `language`     | `CEL`      | *(empty)* |
| `processed-by` | `ferricel` | crate version (e.g. `0.2.0-rc.1`) |

These are merged with entries already present in the embedded runtime template
(contributed by the Rust toolchain), so the final section typically contains:

```text
language:
    Rust
    CEL
processed-by:
    rustc: 1.95.0 (59807616e 2026-04-14)
    walrus: 0.26.1
    ferricel: 0.2.0-rc.1
```

### Inspecting the section

Use [`wasm-tools`](https://github.com/bytecodealliance/wasm-tools) to print the
producers section in a human-readable form:

```sh
wasm-tools metadata show policy.wasm
```

Alternatively, [`wabt`](https://github.com/WebAssembly/wabt)'s `wasm-objdump`
can dump the raw bytes of the section:

```sh
wasm-objdump -s -j producers policy.wasm
```

## Source Custom Sections

Each compiled module embeds the original source and a manifest of host
extensions as raw UTF-8 custom sections, making it possible to recover this
information from a `.wasm` file without any external metadata.

| Section name | Content | Produced by |
|--------------|---------|-------------|
| `ferricel.cel-source` | The original CEL expression | [`compile()`] |
| `ferricel.vap-source` | The full `ValidatingAdmissionPolicy` serialized as YAML | [`compile_vap()`], [`compile_vap_from_policy()`] |
| `ferricel.extensions` | JSON array of host extensions used by this module | all compile paths |

### `ferricel.extensions` section

The `ferricel.extensions` section contains a JSON array of objects, sorted by
`(namespace, function)`, listing every host extension that the module may call
at evaluation time:

```json
[
  { "namespace": null,     "function": "abs"        },
  { "namespace": "kw.k8s", "function": "get"        },
  { "namespace": "kw.k8s", "function": "list"       },
  { "namespace": "kw.net", "function": "lookupHost" }
]
```

`namespace` is `null` for flat (non-namespaced) extensions. The section is
always present; it is an empty array `[]` when the module uses no host
extensions.

The section records extensions that **may** be called — due to CEL's short-circuit
operators (`&&`, `||`), an extension in the list might not be invoked for every
evaluation. A host should use the list to decide which extension implementations
to register, not as a guarantee that all listed extensions will be called.

Read the section at runtime with `ferricel_core::extensions_used`:

```rust
use ferricel_core::extensions_used;

let wasm = std::fs::read("policy.wasm")?;
for ext in extensions_used(&wasm)? {
    println!("{}/{}", ext.namespace.as_deref().unwrap_or("(none)"), ext.function);
}
```

### Inspecting source sections

```sh
# Print the CEL expression embedded in a compiled module
wasm-objdump -s -j ferricel.cel-source policy.wasm

# Print the VAP YAML embedded in a compiled module
wasm-objdump -s -j ferricel.vap-source policy.wasm

# Print the extensions manifest
wasm-objdump -s -j ferricel.extensions policy.wasm
```

With `wasm-tools`, the raw UTF-8 content can be extracted directly:

```sh
wasm-tools dump policy.wasm | grep -A1 "ferricel.cel-source"
```

## Inspecting a Module

The `ferricel inspect` command reads all embedded metadata and prints it in a
human-readable form with syntax highlighting:

```sh
ferricel inspect policy.wasm
```

Output (with color):

```text
Module: policy.wasm

Source (ValidatingAdmissionPolicy):
  apiVersion: admissionregistration.k8s.io/v1
  ...

Host extensions (may be called):
  - kw.k8s/get
  - kw.net/lookupHost

Exports: cel_malloc, evaluate, evaluate_proto
Producers:
  language: CEL, Rust
  processed-by: rustc 1.95.0, walrus 0.26.1, ferricel 0.2.0-rc.1
```

The source is syntax-highlighted: YAML for VAP modules, CEL for plain modules.
The theme is chosen automatically based on the terminal background
(light or dark), using Solarized Light or Solarized Dark respectively.

For machine-readable output, use `--json`:

```sh
ferricel inspect --json policy.wasm
```

Disable color with `--no-color` (e.g. for CI or piping):

```sh
ferricel inspect --no-color policy.wasm
```