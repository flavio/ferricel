# Host Extensions

Host extensions allow compiled CEL expressions to call functions implemented
by the host at evaluation time. Extensions follow a two-phase model:

1. **Compile time** — declare the extension so the compiler emits a real host
   call instead of a static `no matching overload` error.
2. **Runtime** — register an implementation that the Wasm module invokes via
   the `cel_call_extension` host import.

ferricel supports two kinds of host extensions:

- **Flat extensions** — standalone functions like `math.abs(x)` or
  `kw.net.lookupHost(host)`.
- **Builder chains** — fluent APIs like
  `kw.k8s.apiVersion("v1").kind("Pod").list()` where intermediate calls
  accumulate state into a map and a terminal call invokes the host.

## Flat Extensions

### CLI

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

#### `--extensions` format

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

#### `--extensions-file` format

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

### Rust API

Use `ExtensionDecl` to declare the extension at compile time, and
`with_extension` to provide the implementation at runtime:

```rust
use ferricel_core::{compiler, runtime};
use ferricel_types::extensions::ExtensionDecl;

// Declare the extension at compile time
let abs_decl = ExtensionDecl {
    namespace: None,
    function: "abs".to_string(),
    receiver_style: false,
    global_style: true,
    num_args: 1,
};

// Compile the CEL expression with the extension
let wasm = compiler::Builder::new()
    .with_extension(abs_decl.clone())
    .build()
    .compile("abs(x)")?;

// Register the implementation at runtime
let result = runtime::Builder::new()
    .with_extension(abs_decl, |args| {
        let n = args[0].as_i64().unwrap_or(0);
        Ok(serde_json::Value::Number(n.abs().into()))
    })
    .with_wasm(wasm)
    .build()?
    .eval(Some(r#"{"x": -42}"#))?;

assert_eq!(result, "42");
```

The extension declaration specifies the function signature at compile time (for
validation), and the implementation is provided at runtime. The host is
responsible for marshalling JSON values to and from the extension function.

### Dotted namespaces

Namespaces can contain dots, e.g. `kw.net.lookupHost`. The compiler resolves
the full dotted target name from the CEL AST, so both single-segment (`math`)
and multi-segment (`kw.net`) namespaces work identically:

```rust
let decl = ExtensionDecl {
    namespace: Some("kw.net".to_string()),
    function: "lookupHost".to_string(),
    receiver_style: false,
    global_style: true,
    num_args: 1,
};
```

> **Note:** extensions declared at compile time but not implemented by the host
> produce a runtime error when the expression is evaluated.

## Builder Chains

Builder chains model fluent APIs where intermediate method calls accumulate
state into a map and a terminal call invokes the host with the accumulated
state. This is the pattern used by cel-go libraries like `kw.k8s` and
`kw.sigstore`.

### How it works

At runtime, each intermediate builder object is a `CelValue::Object` (map)
with a reserved `"__type__"` key that tracks the builder's current type. For
example, after `kw.k8s.apiVersion("v1").kind("Pod").namespace("default")` the
map is:

```json
{
  "__type__": "kw.k8s.Client",
  "apiVersion": "v1",
  "kind": "Pod",
  "namespace": "default"
}
```

When a terminal method like `.list()` or `.get("nginx")` is called, the host
receives this map as the single argument of the extension call.

### Declaring a builder chain

A builder chain is declared with `BuilderChainDecl`, which contains a list of
`BuilderStep` variants. Register it on the compiler builder with
`with_builder_chain`:

```rust
use ferricel_core::compiler;
use ferricel_types::extensions::{BuilderChainDecl, BuilderStep};

let chain = BuilderChainDecl {
    steps: vec![
        BuilderStep::Entry { /* ... */ },
        BuilderStep::Chain { /* ... */ },
        BuilderStep::Terminal { /* ... */ },
    ],
};

let wasm = compiler::Builder::new()
    .with_builder_chain(chain)
    .build()
    .compile("my.api.start('val').method('arg').run()")?;
```

### Step types

#### Entry

A global entry-point that starts a new chain. Called as a dotted global
function (e.g. `kw.k8s.apiVersion("v1")`).

```rust
BuilderStep::Entry {
    function: "kw.k8s.apiVersion".into(),
    state_keys: vec!["apiVersion".into()],
    output_type: "kw.k8s.ClientBuilder".into(),
}
```

| Field | Description |
|---|---|
| `function` | Full dotted CEL function name. |
| `state_keys` | JSON keys under which each positional argument is stored. The number of keys determines the expected arity. |
| `output_type` | Type tag written to `"__type__"` in the output map. |

#### Chain

A receiver-style chaining step that stores positional arguments under fixed
keys. Supports single-arg and multi-arg steps.

```rust
// Single-arg: .kind("Pod")
BuilderStep::Chain {
    function: "kind".into(),
    input_type: "kw.k8s.ClientBuilder".into(),
    state_keys: vec!["kind".into()],
    output_type: "kw.k8s.Client".into(),
    accumulate: false,
}

// Multi-arg: .keyless("https://issuer", "user@example.com")
BuilderStep::Chain {
    function: "keyless".into(),
    input_type: "sig.VerifierBuilder".into(),
    state_keys: vec!["issuer".into(), "subject".into()],
    output_type: "sig.KeylessVerifier".into(),
    accumulate: false,
}
```

| Field | Description |
|---|---|
| `function` | Method name. |
| `input_type` | Expected `"__type__"` tag of the receiver. Used for compile-time disambiguation. |
| `state_keys` | JSON keys for each positional argument. Determines arity. |
| `output_type` | Type tag for the output map. |
| `accumulate` | When `true`, values are appended to an array under each key instead of overwriting (e.g. repeated `.fieldMask()` calls). |

#### MapEntry

A receiver-style step that inserts a **runtime key/value pair** into a nested
map. Always takes exactly 2 arguments: arg0 is the map key, arg1 is the value.
Repeated calls merge into the same nested map.

```rust
// .annotation("env", "prod") → annotations["env"] = "prod"
BuilderStep::MapEntry {
    function: "annotation".into(),
    input_type: "sig.VerifierBuilder".into(),
    state_key: "annotations".into(),
    output_type: "sig.VerifierBuilder".into(),
}
```

After `.annotation("env", "prod").annotation("team", "sec")`, the state map
contains:

```json
{
  "__type__": "sig.VerifierBuilder",
  "annotations": { "env": "prod", "team": "sec" }
}
```

| Field | Description |
|---|---|
| `function` | Method name. |
| `input_type` | Expected `"__type__"` tag of the receiver. |
| `state_key` | Field name of the nested map in the state object. |
| `output_type` | Type tag for the output map. |

> **Note:** the choice between `Chain` and `MapEntry` is made by the extension
> author, not inferred. The CEL call site looks identical — `.annotation("env",
> "prod")` could be either — but the semantics differ: `Chain` stores arguments
> under fixed positional keys; `MapEntry` uses arg0 as a dynamic key into a
> nested map and accumulates across repeated calls.

#### Terminal

A terminal step that invokes the host with the accumulated state map. Extra
positional arguments (if any) are folded into the map before the call.

```rust
// Zero-arg terminal: .list()
BuilderStep::Terminal {
    function: "list".into(),
    input_type: "kw.k8s.Client".into(),
    extra_arg_keys: vec![],
    host_namespace: "kw.k8s".into(),
    host_function: "list".into(),
}

// One-arg terminal: .get("nginx") — folds "name" into the map
BuilderStep::Terminal {
    function: "get".into(),
    input_type: "kw.k8s.Client".into(),
    extra_arg_keys: vec!["name".into()],
    host_namespace: "kw.k8s".into(),
    host_function: "get".into(),
}
```

| Field | Description |
|---|---|
| `function` | Method name. |
| `input_type` | Expected `"__type__"` tag of the receiver. |
| `extra_arg_keys` | Keys for extra positional arguments folded into the map before the host call. Empty for zero-arg terminals. |
| `host_namespace` | Namespace in the `ExtensionCallPayload` sent to the host. |
| `host_function` | Function name in the `ExtensionCallPayload` sent to the host. |

The host must also register a flat `ExtensionDecl` for each terminal so the
runtime can dispatch the call:

```rust
let ext = ExtensionDecl {
    namespace: Some("kw.k8s".to_string()),
    function: "list".to_string(),
    global_style: false,
    receiver_style: false,
    num_args: 1,  // the accumulated map is the single argument
};
```

### Disambiguation

When multiple chains register the same method name (e.g. both `kw.sigstore`
and `kw.crypto` define a `.verify()` terminal), the compiler disambiguates at
compile time using two criteria:

1. **Receiver type** — each step declares an `input_type`. The compiler tracks
   the static `__type__` of the receiver expression through the chain and
   selects the step whose `input_type` matches.

2. **Argument count** — steps with the same method name and receiver type but
   different arities (e.g. `.githubAction("owner")` vs
   `.githubAction("owner", "repo")`) are distinguished by the number of
   positional arguments, determined by `state_keys.len()` for Chain steps and
   `extra_arg_keys.len()` for Terminal steps.

If disambiguation produces zero or more than one candidate, the compiler
reports an error.

### Complete example

This example declares a small query builder chain, compiles an expression that
uses it, and evaluates it with a host implementation that receives the
accumulated state map:

```rust
use ferricel_core::{compiler, runtime};
use ferricel_types::extensions::{BuilderChainDecl, BuilderStep, ExtensionDecl};

// A tiny query API:  query.field("age").between(18, 65).count()
let chain = BuilderChainDecl {
    steps: vec![
        BuilderStep::Entry {
            function: "query.field".into(),
            state_keys: vec!["field".into()],
            output_type: "query.Builder".into(),
        },
        // Multi-arg step: stores both bounds under "min" and "max".
        BuilderStep::Chain {
            function: "between".into(),
            input_type: "query.Builder".into(),
            state_keys: vec!["min".into(), "max".into()],
            output_type: "query.Range".into(),
            accumulate: false,
        },
        BuilderStep::Terminal {
            function: "count".into(),
            input_type: "query.Range".into(),
            extra_arg_keys: vec![],
            host_namespace: "query".into(),
            host_function: "count".into(),
        },
    ],
};

// Flat extension decl for the terminal
let count_ext = ExtensionDecl {
    namespace: Some("query".to_string()),
    function: "count".to_string(),
    global_style: false,
    receiver_style: false,
    num_args: 1,
};

// Compile
let wasm = compiler::Builder::new()
    .with_builder_chain(chain)
    .build()
    .compile("query.field('age').between(18, 65).count()")?;

// Run
let result = runtime::Builder::new()
    .with_extension(count_ext, |args| {
        // args[0] is the accumulated state map:
        // { "__type__": "query.Range",
        //   "field": "age", "min": 18, "max": 65 }
        let map = &args[0];
        assert_eq!(map["field"], "age");
        assert_eq!(map["min"], 18);
        assert_eq!(map["max"], 65);
        // A real host would run the query and return the matching row count.
        Ok(serde_json::json!(42))
    })
    .with_wasm(wasm)
    .build()?
    .eval(None)?;
```

For a real-world example, see the built-in [`kw.k8s` builder chain](./run-vap-wasm.md#fetching-data-from-the-kubernetes-api)
used by ValidatingAdmissionPolicy support.

## Inspecting Used Extensions

The compiler embeds a `ferricel.extensions` custom section into every compiled
Wasm module. It contains a JSON array listing every host extension the module
**may** call at evaluation time — one entry per unique `(namespace, function)`
pair, sorted and deduplicated.

```json
[
  { "namespace": null,     "function": "abs"        },
  { "namespace": "kw.k8s", "function": "get"        },
  { "namespace": "kw.net", "function": "lookupHost" }
]
```

The section is always present; it is an empty array `[]` for modules that use no
host extensions.

> **Note:** because CEL's `&&` and `||` operators do not short-circuit at
> compile time, an extension listed here may not be called for every evaluation.
> The list records what the module *can* call, not what it *will* call.

### Reading the section from Rust

Use `ferricel_core::extensions_used` to read the section back:

```rust
use ferricel_core::extensions_used;

let wasm = std::fs::read("policy.wasm")?;
for ext in extensions_used(&wasm)? {
    println!("{}/{}", ext.namespace.as_deref().unwrap_or("(none)"), ext.function);
}
```

Returns an empty `Vec` if the section is absent (e.g. modules produced by an
older version of ferricel).

### Reading the section from the command line

Use `ferricel inspect` for a human-readable view of all embedded metadata,
including the extensions list with syntax-highlighted source:

```sh
ferricel inspect policy.wasm
```

Or for just the raw extensions JSON:

```sh
wasm-objdump -s -j ferricel.extensions policy.wasm
```

For the full specification of all custom sections ferricel embeds, and
documentation of the `ferricel inspect` command, see the
[Wasm Spec](./wasm-spec.md#source-custom-sections) chapter.
