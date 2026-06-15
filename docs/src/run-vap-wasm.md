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

## Required Bindings

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

### Fetching Data from the Kubernetes API

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
