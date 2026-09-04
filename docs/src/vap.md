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
message is generated from the validation `expression` text. If the
`messageExpression` produces a runtime error or a non-string value, the module
falls back to the `message` field (or the default message), as Kubernetes does.

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

## Runtime Errors

A CEL expression can fail at runtime: division by zero, a missing field, an
unbound variable, or a host extension (such as `kw.k8s`) that returns an error.

When a `matchConditions` or `validations` expression evaluates to a runtime
error, the compiled module does **not** return `{"accepted": true}` or a
rejection. Instead it traps, exactly like a plain CEL module does: the call to
`evaluate` fails and the host receives an error (`Engine::eval()` returns
`Err`) whose message starts with `CEL runtime error:`. The host decides what
to do with it, which is where `failurePolicy` applies: `Fail` denies the
request, `Ignore` allows it.

Two cases do not trap:

- A `variables` entry that evaluates to an error is stored as-is. The error
  propagates only into the expressions that reference `variables.<name>`. An
  erroring variable that no expression uses is harmless. This matches the lazy
  evaluation of variables in Kubernetes.
- Errors absorbed by CEL short-circuit operators are not errors. For example,
  `(1 / 0) == 1 || true` evaluates to `true`.

The `params` lookup follows the first rule: if the host's `kw.k8s` extension
fails, the error surfaces from the first validation that reads `params`.

## Known Limitations

The following VAP features are not yet implemented or are not part of ferricel's scope:

| Feature                 | Status          | Notes                                                                                                                                       |
| ----------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `failurePolicy`         | Out of scope    | The module traps on a runtime error (see [Runtime Errors](#runtime-errors)). The host maps that error to deny (`Fail`) or allow (`Ignore`). |
| `auditAnnotations`      | Not implemented | Requires a separate compilation path and an additional field in the response JSON.                                                          |
| `matchConstraints`      | Out of scope    | This is a server-side filter applied by the API server, not a CEL expression. The compiled module does not enforce it.                      |
