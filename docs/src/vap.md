# Kubernetes ValidatingAdmissionPolicy Support

ferricel can compile a Kubernetes
[`ValidatingAdmissionPolicy`](https://kubernetes.io/docs/reference/access-authn-authz/validating-admission-policy/)
(VAP) into a self-contained WebAssembly module. The host that runs the Wasm module is
responsible for supplying Kubernetes data, like the namespace object and param resources,
via bindings and extensions.

This is a `ferricel-core` library feature. Enable it with the `k8s-vap` Cargo
feature.

## Response Shape

The module's `evaluate` export returns a JSON object.

Request accepted:

```json
{ "accepted": true }
```

Or, on rejection:

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

## Known Limitations

The following VAP features are not yet implemented or are not part of ferricel's scope:

| Feature                 | Status          | Notes                                                                                                                  |
| ----------------------- | --------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `failurePolicy: Ignore` | Out of scope    | This has to be handled by the host.                                                                                    |
| `auditAnnotations`      | Not implemented | Requires a separate compilation path and an additional field in the response JSON.                                     |
| `matchConstraints`      | Out of scope    | This is a server-side filter applied by the API server, not a CEL expression. The compiled module does not enforce it. |
