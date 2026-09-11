# Running a Compiled ValidatingAdmissionPolicy

Use `runtime::Builder` exactly as you would for any other compiled CEL module.
Pass the variable bindings as a JSON object:

```rust
use ferricel_core::{compiler::Builder, runtime};

let wasm_bytes = Builder::new().build().compile_vap(&yaml)?;

let result_str = runtime::Builder::new()
    .with_wasm(wasm_bytes)
    .build()?
    .eval(Some(&bindings_json))?;

let result: serde_json::Value = serde_json::from_str(&result_str)?;
// result["accepted"] == true / false
```

`build()` (and `build_pre()`) check the module's ABI version before they
link it. A module compiled by a ferricel with an incompatible ABI makes
`build()` return `Err`, naming both ABI versions. See
[ABI Version](wasm-spec.md#abi-version) in the Wasm Spec chapter.

## Runtime Errors and `failurePolicy`

`eval()` returns `Err` when a `matchConditions` or `validations` expression
fails at runtime (for example, division by zero, an unbound variable, or a
`kw.k8s` extension that returns an error). The error downcasts to
`ferricel_core::CelRuntimeError`. Its `Display` text starts with
`CEL runtime error:`. The module never turns such a failure into an accept or
a reject response.

`eval()` can also fail for reasons that are not CEL runtime errors: an
epoch-deadline interrupt, a memory limit, a Wasm trap, or a bug in a host
extension. These errors do not downcast to `CelRuntimeError`.

The host is responsible for applying the policy's `failurePolicy`. With
`downcast_ref`, the host can tell a CEL runtime error apart from other
failures and decide how each kind maps to `failurePolicy`:

```rust
use ferricel_core::CelRuntimeError;

match engine.eval(Some(&bindings_json)) {
    Ok(result_str) => {
        let result: serde_json::Value = serde_json::from_str(&result_str)?;
        // result["accepted"] == true / false
    }
    Err(err) => match err.downcast_ref::<CelRuntimeError>() {
        Some(cel_err) => {
            // The CEL expression evaluated to an error. Apply failurePolicy:
            //   Fail   -> deny the request, report `cel_err.message`
            //   Ignore -> allow the request
        }
        None => {
            // Not a CEL runtime error: a deadline, a resource limit, a trap,
            // or a host bug. The host decides how to handle it.
        }
    },
}
```

`CelRuntimeError` has two fields:

| Field     | Type                      | Description                                                          |
| --------- | ------------------------- | -------------------------------------------------------------------- |
| `message` | `String`                  | The error message, for example `divide by zero`.                     |
| `origin`  | `Option<ExtensionOrigin>` | The host extension that produced the error, or `None`.               |

`ExtensionOrigin` holds the `namespace` (for example `Some("kw.k8s")`) and
the `function` (for example `get`) of the extension call. When the `params`
lookup fails in the host, `origin` is `kw.k8s.get` or `kw.k8s.list`. A host
can use this to tell a failed `params` lookup apart from other runtime errors.

See [Runtime Errors](vap.md#runtime-errors) for the exact rules.

## Required Bindings

| Binding           | Required when…                                                   |
| ----------------- | ---------------------------------------------------------------- |
| `object`          | Always (the resource being admitted)                             |
| `oldObject`       | Policy expressions reference `oldObject`                         |
| `request`         | Policy expressions reference `request`, or `paramKind` is set    |
| `namespaceObject` | Policy expressions reference `namespaceObject`                   |
| `paramRef`        | `paramKind` is set (see below)                                   |

`object`, `oldObject`, and `request` correspond directly to the fields of the
Kubernetes
[`AdmissionReview`](https://kubernetes.io/docs/reference/access-authn-authz/extensible-admission-controllers/#request)
request object.

Determining whether a given policy actually references `oldObject`,
`request`, or `namespaceObject` — without evaluating the policy or re-parsing
its CEL/YAML source — is exactly what the `ferricel.vap-variables` custom
section is for. Read it with `ferricel_core::vap_variables_used()` (or
`ferricel inspect --json`) at policy-setup time and only fetch/bind what is
actually needed. This is particularly relevant for `namespaceObject`, since
fetching it requires an extra host-side lookup — see
[Custom sections and inspection](wasm-spec.md#ferricelvap-variables-section)
for details.

## Kubernetes Resource Fetching

### Params

When a policy sets `paramKind`, the compiled module fetches the param
resources itself at evaluation time. It calls the host-provided `kw.k8s.get`
or `kw.k8s.list` extension. The host does **not** supply `params` directly in
the bindings.

The host must supply `paramRef` in the bindings. `paramRef` is the
`spec.paramRef` of the `ValidatingAdmissionPolicyBinding`, as JSON. The host
must also supply `request`, so the module can default the namespace (see
below).

The module supports the two ways Kubernetes selects param resources:

- `paramRef.name`: one resource, by name. The module calls `kw.k8s.get`.
- `paramRef.selector`: every resource that matches a label selector. The
  module formats the selector as a label selector string, for example
  `app=web,tier in (api,web)`, and calls `kw.k8s.list`. Each item in the
  `items` list of the response is one param resource.

If `paramRef` has both `name` and `selector`, `name` wins. If it has neither,
the module traps with the error `paramRef must have either name or selector`.

#### Namespace defaulting

The `namespace` in the request map is `paramRef.namespace`. If that is empty,
the module uses `request.namespace`. If that is also empty, `namespace` is
`""`. The params request map **always** contains the `namespace` key. This is
different from a `kw.k8s` chain written in CEL, where `namespace` is present
only when the policy calls `.namespace()`.

#### Per-param evaluation

The module evaluates the policy once per param resource. For each param, it
sets the `params` binding, then evaluates `matchConditions`, `variables`, and
`validations`. The first rejection stops the evaluation and becomes the
response. When every param passes, or the param list is empty, the module
returns `{"accepted": true}`. See [Evaluation Order](vap.md#evaluation-order).

#### `parameterNotFoundAction`

When the host call fails, or `kw.k8s.list` returns zero items, the module
reads `paramRef.parameterNotFoundAction`:

- `Allow`: the param list is empty. The module returns `{"accepted": true}`.
- `Deny`, or unset: the module traps with a `CelRuntimeError`. The host then
  applies its `failurePolicy`. A host error keeps its origin (`kw.k8s.get` or
  `kw.k8s.list`). An empty list produces the message `no parameters found`.

Under `Allow`, a host error of any kind counts as "not found". The module
cannot tell a missing resource apart from an authorization error. See
[LIMITATIONS.md](https://github.com/flavio/ferricel/blob/main/LIMITATIONS.md).

#### Bindings and host registration

A binding with `paramRef.name`:

```json
{
  "paramRef": {
    "name": "my-params",
    "namespace": "default",
    "parameterNotFoundAction": "Deny"
  },
  "request": { "namespace": "team-a", ... },
  "object": { ... }
}
```

A binding with `paramRef.selector`:

```json
{
  "paramRef": {
    "selector": {
      "matchLabels": { "app": "web" },
      "matchExpressions": [
        { "key": "tier", "operator": "In", "values": ["api", "web"] }
      ]
    },
    "parameterNotFoundAction": "Allow"
  },
  "request": { "namespace": "team-a", ... },
  "object": { ... }
}
```

The host must register both `kw.k8s.get` and `kw.k8s.list` on the runtime
builder. Which one the module calls depends on the binding, not on the
policy:

```rust
use ferricel_core::{compiler::Builder, runtime, compiler::vap};

let wasm_bytes = Builder::new().build().compile_vap(&yaml)?;

let result_str = runtime::Builder::new()
    .with_wasm(wasm_bytes)
    .with_extension(vap::kw_k8s_get_extension(), |args| {
        // args[0] is the request map (see shape below)
        let map = &args[0];
        let name        = map["name"].as_str().unwrap();
        let namespace   = map["namespace"].as_str().unwrap(); // can be ""
        let api_version = map["apiVersion"].as_str().unwrap();
        let kind        = map["kind"].as_str().unwrap();

        // Fetch from Kubernetes and return the resource as a JSON value.
        let resource = fetch_from_k8s(api_version, kind, namespace, name)?;
        Ok(resource)
    })
    .with_extension(vap::kw_k8s_list_extension(), |args| {
        let map = &args[0];
        let label_selector = map["labelSelector"].as_str().unwrap();
        let namespace      = map["namespace"].as_str().unwrap(); // can be ""
        let api_version    = map["apiVersion"].as_str().unwrap();
        let kind           = map["kind"].as_str().unwrap();

        // List from Kubernetes and return `{"items": [...]}`.
        let list = list_from_k8s(api_version, kind, namespace, label_selector)?;
        Ok(list)
    })
    .build()?
    .eval(Some(&bindings_json))?;
```

### Fetching Data from the Kubernetes API

> The `kw.k8s` API is implemented as a
> [builder chain](./host-extensions.md#builder-chains). See the
> [Host Extensions](./host-extensions.md) chapter for general documentation on
> declaring and consuming builder chains.

Policy `variables` (and other expressions) can call `kw.k8s` directly to fetch
arbitrary resources. The API mirrors the
[`kw.k8s` Kubernetes library](https://pkg.go.dev/github.com/kubewarden/policies/policies/cel-policy/internal/cel/library#Kubernetes)
provided by the [Kubewarden CEL policy](https://github.com/kubewarden/policies/tree/main/policies/cel-policy):

```text
kw.k8s
  .apiVersion(<string>)     → kw.k8s.ClientBuilder
  .kind(<string>)           → kw.k8s.Client
  .namespace(<string>)      → kw.k8s.Client   (optional)
  .labelSelector(<string>)  → kw.k8s.Client   (optional)
  .fieldSelector(<string>)  → kw.k8s.Client   (optional)
  .fieldMask(<string>)      → kw.k8s.Client   (optional, repeatable)
  .get(<string>)            → dyn              (host call — returns one resource)
  .list()                   → dyn              (host call — returns a list)
```

Example — fetch a ConfigMap in a variable, then check a field in a validation:

```cel
// variables entry
kw.k8s.apiVersion('v1').kind('ConfigMap').namespace('default').get('my-config')

// validation expression
variables.cfg.data.allowedTeam == request.userInfo.groups[0]
```

### Host Extension Request Map

When a `kw.k8s.get` or `kw.k8s.list` terminal is called, the host receives a
single argument — a JSON object containing the accumulated builder state:

| Key             | Set by chain step  | Notes                                     |
| --------------- | ------------------ | ----------------------------------------- |
| `apiVersion`    | `.apiVersion()`    | Always present                            |
| `kind`          | `.kind()`          | Always present                            |
| `namespace`     | `.namespace()`     | Present only if `.namespace()` was called |
| `labelSelector` | `.labelSelector()` | Present only if called                    |
| `fieldSelector` | `.fieldSelector()` | Present only if called                    |
| `fieldMasks`    | `.fieldMask()`     | Array; present only if called             |
| `name`          | `.get(<name>)`     | Present only for `get` terminal           |

For the `params` lookup, the module builds this map itself. It always sets
`apiVersion`, `kind`, and `namespace`. It sets `name` for `paramRef.name` and
`labelSelector` for `paramRef.selector`. See [Params](#params).

Register the extensions using the helpers from `ferricel_core::compiler::vap`:

```rust
use ferricel_core::compiler::vap;

// For policies that call .get(...), or set paramKind
runtime::Builder::new()
    .with_extension(vap::kw_k8s_get_extension(), |args| { ... })

// For policies that call .list(), or set paramKind
runtime::Builder::new()
    .with_extension(vap::kw_k8s_list_extension(), |args| { ... })
```

## Example

This example mirrors the scenario from the
[Kubewarden CEL policy README](https://github.com/kubewarden/policies/blob/main/policies/cel-policy/README.md):
a policy that enforces a maximum replica count read from a `ConfigMap` parameter resource.

### The `ValidatingAdmissionPolicyBinding`

```yaml
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicyBinding
metadata:
  name: "demo-policy-binding.example.com"
spec:
  policyName: "demo-policy.example.com"
  validationActions: [Deny]
  paramRef:
    name: "my-params"
    namespace: "default"
    parameterNotFoundAction: Deny
  matchResources:
    namespaceSelector:
      matchLabels:
        environment: test
```

### The `ConfigMap` Parameter Resource

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: my-params
  namespace: default
data:
  maxreplicas: "5"
```

### The Incoming `Deployment`

This is the resource being admitted:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: my-app
  namespace: default
spec:
  replicas: 3
  selector:
    matchLabels:
      app: my-app
  template:
    metadata:
      labels:
        app: my-app
    spec:
      containers:
        - name: my-app
          image: my-app:latest
```

### Rust Integration

```rust
use ferricel_core::{compiler::{Builder, vap}, runtime};

// The host extracts these from the AdmissionReview and the PolicyBinding.
let bindings = serde_json::json!({
    "paramRef": {
        "name": "my-params",
        "namespace": "default",
        "parameterNotFoundAction": "Deny"
    },
    "request": request_json,
    "object": object_json,
});

let wasm_bytes = Builder::new().build().compile_vap(vap_yaml)?;

let result_str = runtime::Builder::new()
    .with_wasm(wasm_bytes)
    .with_extension(vap::kw_k8s_get_extension(), |args| {
        // The module calls this to fetch the ConfigMap.
        // In production, make a real Kubernetes API call here.
        let map = &args[0];
        assert_eq!(map["apiVersion"], "v1");
        assert_eq!(map["kind"], "ConfigMap");
        assert_eq!(map["name"], "my-params");
        assert_eq!(map["namespace"], "default");
        Ok(serde_json::json!({
            "apiVersion": "v1",
            "kind": "ConfigMap",
            "metadata": { "name": "my-params", "namespace": "default" },
            "data": { "maxreplicas": "5" }
        }))
    })
    .with_extension(vap::kw_k8s_list_extension(), |args| {
        // Not called for this binding, because `paramRef.name` is set.
        // A binding with `paramRef.selector` calls this instead.
        unreachable!("this binding uses paramRef.name")
    })
    .build()?
    .eval(Some(&bindings.to_string()))?;

let result: serde_json::Value = serde_json::from_str(&result_str)?;
assert_eq!(result["accepted"], true);  // replicas 3 <= maxreplicas 5
```
