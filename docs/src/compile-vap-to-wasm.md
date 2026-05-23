# Compiling a ValidatingAdmissionPolicy to Wasm

ferricel can compile a Kubernetes
[`ValidatingAdmissionPolicy`](https://kubernetes.io/docs/reference/access-authn-authz/validating-admission-policy/)
(VAP) into a self-contained WebAssembly module. The host that runs the Wasm module is
responsible for supplying Kubernetes data, like the namespace object and param resources,
via bindings and extensions.

This is a `ferricel-core` library feature. Enable it with the `k8s-vap` Cargo
feature.

## Compilation

### From YAML

Pass the raw YAML string to `compile_vap`:

```rust
use ferricel_core::compiler::Builder;

let yaml = std::fs::read_to_string("policy.yaml")?;
let wasm_bytes = Builder::new().build().compile_vap(&yaml)?;
```

The YAML must contain exactly one `ValidatingAdmissionPolicy` document.

### From a Parsed Spec

The `ValidatingAdmissionPolicySpec` type is defined by the
[`k8s-openapi`](https://crates.io/crates/k8s-openapi) crate.

```rust
use ferricel_core::compiler::Builder;
use k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicySpec;

let spec: ValidatingAdmissionPolicySpec = /* ... */;
let wasm_bytes = Builder::new().build().compile_vap_from_spec(&spec)?;
```

## Response Shape

The module's `evaluate` export returns a JSON object:

Request accepted:

```json
{ "accepted": true }
```

or, on rejection:

```json
{ "accepted": false, "message": "too many replicas", "code": 422 }
```

The `message` field comes from the failing validation's `message` field, or from
its `messageExpression` if one is specified. If neither is set, a default
message is generated from the validation `expression` text.

The `code` field is derived from the validation's `reason` field:

| `reason`                | HTTP code |
| ----------------------- | --------- |
| `Forbidden`             | 403       |
| `Unauthorized`          | 401       |
| `RequestEntityTooLarge` | 413       |
| `Invalid` or unset      | 422       |

## Evaluation Order

The compiled module enforces the standard Kubernetes VAP evaluation order:

1. **`matchConditions`** — evaluated in declaration order. If any condition
   evaluates to `false`, the policy does **not** apply to this request: the
   module returns `{"accepted": true}` immediately (a skip, not a rejection).
   Remaining `matchConditions` and all `validations` are not evaluated.

2. **`variables`** — evaluated in declaration order. Each result is stored
   under `variables.<name>` and is immediately accessible to subsequent
   `variables` expressions and to all `validations`.

3. **`validations`** — evaluated in declaration order. The first expression
   that evaluates to `false` causes the module to return a rejection response.
   Remaining validations are not evaluated.

## Running the Compiled Module

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

### Required Bindings

| Binding           | Required when…                                 |
| ----------------- | ---------------------------------------------- |
| `object`          | Always (the resource being admitted)           |
| `oldObject`       | Policy expressions reference `oldObject`       |
| `request`         | Policy expressions reference `request`         |
| `namespaceObject` | Policy expressions reference `namespaceObject` |
| `paramRef`        | `paramKind` is set (see below)                 |

`object`, `oldObject`, and `request` correspond directly to the fields of the
Kubernetes
[`AdmissionReview`](https://kubernetes.io/docs/reference/access-authn-authz/extensible-admission-controllers/#request)
request object.

## Kubernetes Resource Fetching

### Params

When a policy sets `paramKind`, the compiled module fetches the referenced
resource itself at evaluation time by calling a host-provided `kw.k8s.get`
extension. The host does **not** supply `params` directly in the bindings.

The module reads `paramRef.name` and `paramRef.namespace` from the bindings at
runtime and forwards them to the host as part of the request map (see below).
The result is stored in `params` and made available to all `variables` and
`validations` expressions.

The host must supply `paramRef` in the bindings:

```json
{
  "paramRef": { "name": "my-params", "namespace": "default" },
  "object": { ... }
}
```

And register a `kw.k8s.get` implementation on the runtime builder:

```rust
use ferricel_core::{compiler::Builder, runtime, compiler::vap};

let wasm_bytes = Builder::new().build().compile_vap(&yaml)?;

let result_str = runtime::Builder::new()
    .with_wasm(wasm_bytes)
    .with_extension(vap::kw_k8s_get_extension(), |args| {
        // args[0] is the accumulated request map (see shape below)
        let map = &args[0];
        let name        = map["name"].as_str().unwrap();
        let namespace   = map["namespace"].as_str().unwrap();
        let api_version = map["apiVersion"].as_str().unwrap();
        let kind        = map["kind"].as_str().unwrap();

        // Fetch from Kubernetes and return the resource as a JSON value.
        let resource = fetch_from_k8s(api_version, kind, namespace, name)?;
        Ok(resource)
    })
    .build()?
    .eval(Some(&bindings_json))?;
```

### Fetching data from Kubernetes API

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

Register the extensions using the helpers from `ferricel_core::compiler::vap`:

```rust
use ferricel_core::compiler::vap;

// For policies that call .get(...)
runtime::Builder::new()
    .with_extension(vap::kw_k8s_get_extension(), |args| { ... })

// For policies that call .list()
runtime::Builder::new()
    .with_extension(vap::kw_k8s_list_extension(), |args| { ... })
```

## Complete Example

The following example mirrors the scenario from the
[Kubewarden CEL policy README](https://github.com/kubewarden/policies/blob/main/policies/cel-policy/README.md):
a policy that enforces a maximum replica count read from a `ConfigMap` parameter resource.

### The `ValidatingAdmissionPolicy`

```yaml
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: "demo-policy.example.com"
spec:
  failurePolicy: Fail
  paramKind:
    apiVersion: v1
    kind: ConfigMap
  matchConstraints:
    resourceRules:
      - apiGroups: ["apps"]
        apiVersions: ["v1"]
        operations: ["CREATE", "UPDATE"]
        resources: ["deployments"]
  variables:
    - name: replicas
      expression: "object.spec.replicas"
  validations:
    - expression: "variables.replicas <= int(params.data.maxreplicas)"
      message: "The number of replicas must be less than or equal to the configured maximum"
```

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
    "paramRef": { "name": "my-params", "namespace": "default" },
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
    .build()?
    .eval(Some(&bindings.to_string()))?;

let result: serde_json::Value = serde_json::from_str(&result_str)?;
assert_eq!(result["accepted"], true);  // replicas 3 <= maxreplicas 5
```

---

## Known Limitations

The following VAP features are not yet implemented or are not part of ferricel's scope:

| Feature                 | Status          | Notes                                                                                                                  |
| ----------------------- | --------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `failurePolicy: Ignore` | Out of scope    | This has to be handled by the host.                                                                                    |
| `auditAnnotations`      | Not implemented | Requires a separate compilation path and an additional field in the response JSON.                                     |
| `matchConstraints`      | Out of scope    | This is a server-side filter applied by the API server, not a CEL expression. The compiled module does not enforce it. |
