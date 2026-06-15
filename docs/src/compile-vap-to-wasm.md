# Compiling a ValidatingAdmissionPolicy to Wasm

## From YAML

Pass the raw YAML string to `compile_vap`:

```rust
use ferricel_core::compiler::Builder;

let yaml = std::fs::read_to_string("policy.yaml")?;
let wasm_bytes = Builder::new().build().compile_vap(&yaml)?;
```

The YAML must contain exactly one `ValidatingAdmissionPolicy` document.

## From a Parsed Policy

The `ValidatingAdmissionPolicy` type is defined by the
[`k8s-openapi`](https://crates.io/crates/k8s-openapi) crate.

```rust
use ferricel_core::compiler::Builder;
use k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicy;

let policy: ValidatingAdmissionPolicy = /* ... */;
let wasm_bytes = Builder::new().build().compile_vap_from_policy(&policy)?;
```

## Example

The following policy enforces a maximum replica count read from a `ConfigMap`
parameter resource:

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

Compile it to a Wasm module:

```rust
use ferricel_core::compiler::Builder;

let yaml = std::fs::read_to_string("policy.yaml")?;
let wasm_bytes = Builder::new().build().compile_vap(&yaml)?;
```
