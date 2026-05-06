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