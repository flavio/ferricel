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

The compiled module follows the evaluation order of the Kubernetes VAP
validator:

1. **`params`** (only when `paramKind` is set). The module resolves the list
   of param resources from `paramRef`. See
   [Params](run-vap-wasm.md#params) for the rules. Without `paramKind`, the
   module evaluates the steps below once, with no `params` binding.

2. For each param resource:

   1. **`matchConditions`**. Evaluated in declaration order. If any condition
      evaluates to `false`, this param does **not** apply to the request. The
      module skips the remaining `matchConditions`, the `variables`, and the
      `validations` for this param, and moves to the next param. A skip is
      not a rejection. `params` is available in `matchConditions`.

   2. **`variables`**. Evaluated in declaration order. Each result is stored
      under `variables.<name>`. It is accessible to later `variables`
      expressions and to all `validations`. The module rebuilds the
      `variables` map for each param.

   3. **`validations`**. Evaluated in declaration order. The first expression
      that evaluates to `false` makes the module return a rejection response.
      The module does not evaluate the remaining validations or the remaining
      params.

3. When no param produced a rejection, the module returns
   `{"accepted": true}`. An empty param list also produces this response.

When several params match a `paramRef.selector` and more than one rejects
the request, the module returns the first rejection only. Kubernetes
aggregates every rejection message. See [LIMITATIONS.md](https://github.com/flavio/ferricel/blob/main/LIMITATIONS.md).

## Runtime Errors

A CEL expression can fail at runtime: division by zero, a missing field, an
unbound variable, or a host extension (such as `kw.k8s`) that returns an error.

When a `matchConditions` or `validations` expression evaluates to a runtime
error, the compiled module does **not** return `{"accepted": true}` or a
rejection. Instead it traps, exactly like a plain CEL module does: the call to
`evaluate` fails and `Engine::eval()` returns `Err`. The error downcasts to
`ferricel_core::CelRuntimeError`. The host decides what to do with it, which
is where `failurePolicy` applies: `Fail` denies the request, `Ignore` allows
it.

`Engine::eval()` can also fail for other reasons: an epoch-deadline
interrupt, a memory limit, a Wasm trap, or a bug in a host extension. These
errors do not downcast to `CelRuntimeError`. The host can tell the two kinds
apart. How each kind maps to `failurePolicy` is the host's decision. See
[Runtime Errors and `failurePolicy`](run-vap-wasm.md#runtime-errors-and-failurepolicy)
for the host-side code.

Two cases do not trap:

- A `variables` entry that evaluates to an error is stored as-is. The error
  propagates only into the expressions that reference `variables.<name>`. An
  erroring variable that no expression uses is harmless. This matches the lazy
  evaluation of variables in Kubernetes.
- Errors absorbed by CEL short-circuit operators are not errors. For example,
  `(1 / 0) == 1 || true` evaluates to `true`.

The `params` lookup traps before any `matchConditions` or `validations`
run. If the host's `kw.k8s` extension fails, or the lookup finds no
resource, and `paramRef.parameterNotFoundAction` is not `Allow`, the module
traps. For a host error, the `origin` field of the `CelRuntimeError` is
`kw.k8s.get` (for `paramRef.name`) or `kw.k8s.list` (for
`paramRef.selector`). For an empty result, the message is
`no parameters found` and `origin` is `None`. See
[Params](run-vap-wasm.md#params).

## Known Limitations

The following VAP features are not yet implemented or are not part of ferricel's scope:

| Feature                 | Status          | Notes                                                                                                                                       |
| ----------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `failurePolicy`         | Out of scope    | The module traps on a runtime error (see [Runtime Errors](#runtime-errors)). The host maps that error to deny (`Fail`) or allow (`Ignore`). |
| `auditAnnotations`      | Not implemented | Requires a separate compilation path and an additional field in the response JSON.                                                          |
| `matchConstraints`      | Out of scope    | This is a server-side filter applied by the API server, not a CEL expression. The compiled module does not enforce it.                      |
