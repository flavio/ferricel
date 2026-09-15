# Kubernetes ValidatingAdmissionPolicy Support

ferricel can compile a Kubernetes
[`ValidatingAdmissionPolicy`](https://kubernetes.io/docs/reference/access-authn-authz/validating-admission-policy/)
(VAP) into a self-contained WebAssembly module. The compiled module fetches
Kubernetes data it needs — the namespace object and param resources —
itself, through host extensions. The host supplies everything else (the
resource under admission, the admission request) via bindings.

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

Both responses can carry an optional `warnings` list of strings. The list is
present only under `failurePolicy: Ignore`, and only when the module skipped
at least one expression. The module skips an expression when it evaluates to
a runtime error. See [`failurePolicy: Ignore`](#failurepolicy-ignore).

## Evaluation Order

The compiled module follows the evaluation order of the Kubernetes VAP
validator:

1. **`namespaceObject`** (only when the policy references it). The module
   resolves the Namespace of the resource under admission from `request`.
   See [Namespace object](run-vap-wasm.md#namespace-object) for the rules.

2. **`params`** (only when `paramKind` is set). The module resolves the list
   of param resources from `paramRef`. See
   [Params](run-vap-wasm.md#params) for the rules. Without `paramKind`, the
   module evaluates the steps below once, with no `params` binding.

3. For each param resource:

   1. **`matchConditions`**. Evaluated in declaration order. If any condition
      evaluates to `false`, this param does **not** apply to the request. The
      module skips the remaining `matchConditions`, the `variables`, and the
      `validations` for this param, and moves to the next param. A skip is
      not a rejection. `params` and `namespaceObject` are both available in
      `matchConditions`.

   2. **`variables`**. Evaluated in declaration order. Each result is stored
      under `variables.<name>`. It is accessible to later `variables`
      expressions and to all `validations`. The module rebuilds the
      `variables` map for each param.

   3. **`validations`**. Evaluated in declaration order. The first expression
      that evaluates to `false` makes the module return a rejection response.
      The module does not evaluate the remaining validations or the remaining
      params.

4. When no param produced a rejection, the module returns
   `{"accepted": true}`. An empty param list also produces this response.

When several params match a `paramRef.selector` and more than one rejects
the request, the module returns the first rejection only. Kubernetes
aggregates every rejection message. See [LIMITATIONS.md](https://github.com/flavio/ferricel/blob/main/LIMITATIONS.md).

## Runtime Errors

A CEL expression can evaluate to a runtime error. Examples: division by
zero, a missing field, an unbound variable, or a host extension (such as
`kw.k8s`) that returns an error.

The host passes the `failurePolicy` of the policy in the `failurePolicy`
binding (see [Required Bindings](run-vap-wasm.md#required-bindings)). The
value is `"Fail"` or `"Ignore"`. A missing or `null` binding means `Fail`. If
the value is anything else, the module traps with a runtime error before the
first expression runs. The module does not read `spec.failurePolicy` from the
VAP. The host owns that value.

### `failurePolicy: Fail`

When a `matchConditions` or `validations` expression evaluates to a runtime
error, the module does **not** return `{"accepted": true}` or a rejection.
The module traps, exactly like a plain CEL module does. The call to `evaluate`
fails, and `Engine::eval()` returns `Err`. The error downcasts to
`ferricel_core::CelRuntimeError`. The host denies the request.

`Engine::eval()` can also fail for other reasons: an epoch-deadline
interrupt, a memory limit, a Wasm trap, or a bug in a host extension. These
errors do not downcast to `CelRuntimeError`, so the host can tell the two
kinds apart. See
[Runtime Errors and `failurePolicy`](run-vap-wasm.md#runtime-errors-and-failurepolicy)
for the host-side code.

### `failurePolicy: Ignore`

The module applies the policy to each expression on its own, the same way
Kubernetes does:

- When a `validations` expression evaluates to an error, the module skips it.
  The next validation runs. A `false` result from any other validation
  rejects the request.
- When a `matchConditions` expression evaluates to an error, the module skips
  the current param, like for a `false` condition. With `paramKind`, the next
  param runs. Without `paramKind`, the policy does not apply, and the module
  accepts the request.

Each skipped expression adds one entry to the `warnings` list of the
response. The list is present on both accept and reject responses. It is
absent when the module skipped nothing:

```json
{"accepted": true, "warnings": ["The module skipped validation[0] because the expression evaluated to an error (failurePolicy is Ignore): no such key: 'count'"]}
```

```json
{"accepted": false, "message": "...", "code": 422, "warnings": ["The module skipped validation[0] because the expression evaluated to an error (failurePolicy is Ignore): no such key: 'count'"]}
```

The text names the expression (`validation[<i>]` or
`matchCondition '<name>'`) and carries the error message. With `paramKind`,
the text names the param (`params <namespace>/<name>`), so an operator can
tell the params apart. Kubernetes sends no client warning for a skipped
expression. It records a metric and an audit annotation instead. The
warnings are a ferricel addition.

`Ignore` does not cover the `params` and `namespaceObject` lookups. An error
there always traps (see below). `Ignore` also does not cover runtime
functions that abort instead of returning an error value, such as
`int("x")`. See
[LIMITATIONS.md](https://github.com/flavio/ferricel/blob/main/LIMITATIONS.md).

### Errors that are never errors

Two cases are not runtime errors under either policy:

- A `variables` entry that evaluates to an error is stored as-is. The error
  propagates only into the expressions that reference `variables.<name>`. An
  erroring variable that no expression uses is harmless. This matches the lazy
  evaluation of variables in Kubernetes.
- Errors absorbed by CEL short-circuit operators are not errors. For example,
  `(1 / 0) == 1 || true` evaluates to `true`.

### Lookups that always trap

The `params` lookup traps before any `matchConditions` or `validations`
run, under both `Fail` and `Ignore`. If the host's `kw.k8s` extension fails, or the lookup finds no
resource, and `paramRef.parameterNotFoundAction` is not `Allow`, the module
traps. For a host error, the `origin` field of the `CelRuntimeError` is
`kw.k8s.get` (for `paramRef.name`) or `kw.k8s.list` (for
`paramRef.selector`). For an empty result, the message is
`no parameters found` and `origin` is `None`. See
[Params](run-vap-wasm.md#params).

The `namespaceObject` lookup traps before the `params` lookup, and before
any `matchConditions` or `validations` run, when the `request` binding is
missing or the host's `kw.k8s.get` call fails. For a host error, `origin` is
`kw.k8s.get` — the same origin a failed `paramRef.name` lookup uses, so a
host that only checks `origin == kw.k8s.get` cannot tell the two apart from
the error alone. Unlike `params`, there is no `parameterNotFoundAction`
equivalent: a cluster-scoped request, or a request for the Namespace
resource itself, does not trap — it makes `namespaceObject` `null` without
calling the host at all. See
[Namespace object](run-vap-wasm.md#namespace-object).

## Known Limitations

The following VAP features are not yet implemented or are not part of ferricel's scope:

| Feature                 | Status          | Notes                                                                                                                                       |
| ----------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `failurePolicy`         | Implemented     | Via the `failurePolicy` binding (see [Runtime Errors](#runtime-errors)). Under `Ignore`, the module skips an expression that evaluates to an error and records a warning. |
| `auditAnnotations`      | Not implemented | Requires a separate compilation path and an additional field in the response JSON.                                                          |
| `matchConstraints`      | Out of scope    | This is a server-side filter applied by the API server, not a CEL expression. The compiled module does not enforce it.                      |
