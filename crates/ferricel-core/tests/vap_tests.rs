//! Integration tests for VAP (ValidatingAdmissionPolicy) compilation.
//!
//! Each test compiles a VAP `spec:` YAML fragment and executes it with JSON
//! bindings, then asserts the resulting `ValidationResponse`-style JSON (or
//! runtime error) via [`Expected`] / [`assert_outcome`].

#![cfg(feature = "k8s-vap")]

use ferricel_core::{
    CelRuntimeError, ExtensionOrigin,
    compiler::{Builder, vap},
    runtime,
};
use ferricel_types::{LogLevel, extensions::ExtensionDecl};
use rstest::rstest;
use slog::{Drain, Logger, o};

fn test_logger() -> Logger {
    let decorator = slog_term::PlainSyncDecorator::new(std::io::stderr());
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    Logger::root(drain, o!())
}

// ─── YAML / eval helpers ────────────────────────────────────────────────────

/// Wrap a `spec:` YAML fragment (e.g. `"spec:\n  validations:\n    - ...\n"`)
/// in the common `ValidatingAdmissionPolicy` document header. The policy name
/// is irrelevant to any test here.
fn vap_yaml(spec_body: &str) -> String {
    format!(
        r#"apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-policy
{spec_body}"#
    )
}

/// A host implementation for a single extension (e.g. `kw.k8s.get`).
type HostFn =
    Box<dyn Fn(Vec<serde_json::Value>) -> Result<serde_json::Value, String> + Send + Sync>;

/// Compile a VAP `spec:` YAML fragment and evaluate it with the given JSON
/// bindings, returning the parsed `serde_json::Value`.
///
/// `extension`, if set, registers a single host extension implementation
/// (e.g. for `kw.k8s...`) on the `Engine`. For a test that needs more than
/// one extension (e.g. `namespaceObject`'s `kw.k8s.get` together with
/// `params`'s `kw.k8s.list`), use [`eval_vap_with_extensions`].
fn eval_vap(
    spec_body: &str,
    bindings_json: &str,
    extension: Option<(ExtensionDecl, HostFn)>,
) -> Result<serde_json::Value, anyhow::Error> {
    eval_vap_with_extensions(spec_body, bindings_json, extension.into_iter().collect())
}

/// Like [`eval_vap`], but registers every extension in `extensions` on the
/// `Engine`.
fn eval_vap_with_extensions(
    spec_body: &str,
    bindings_json: &str,
    extensions: Vec<(ExtensionDecl, HostFn)>,
) -> Result<serde_json::Value, anyhow::Error> {
    let mut results = eval_vap_repeatedly(spec_body, &[bindings_json], extensions)?;
    results.pop().expect("one evaluation, one result")
}

/// Compile a VAP `spec:` YAML fragment once, then evaluate it once per entry
/// of `bindings` on the **same** `Engine`, in order. Returns one result per
/// evaluation.
///
/// A test uses this function to make sure that per-evaluation state (like
/// the `failurePolicy` warnings) does not leak from one request into the
/// next. The Kubewarden host uses one `Engine` for many requests the same
/// way.
fn eval_vap_repeatedly(
    spec_body: &str,
    bindings: &[&str],
    extensions: Vec<(ExtensionDecl, HostFn)>,
) -> Result<Vec<Result<serde_json::Value, anyhow::Error>>, anyhow::Error> {
    let logger = test_logger();
    let wasm_bytes = Builder::new()
        .with_logger(logger.clone())
        .build()
        .compile_vap(&vap_yaml(spec_body))?;

    let mut runtime_builder = runtime::Builder::new()
        .with_logger(logger)
        .with_log_level(LogLevel::Info)
        .with_wasm(wasm_bytes);
    for (decl, implementation) in extensions {
        runtime_builder = runtime_builder.with_extension(decl, implementation);
    }
    let engine = runtime_builder.build()?;

    Ok(bindings
        .iter()
        .map(|bindings_json| {
            let result_str = engine.eval(Some(bindings_json))?;
            Ok(serde_json::from_str(&result_str)?)
        })
        .collect())
}

// ─── Expected outcome + single assertion ───────────────────────────────────

/// The possible outcomes of evaluating a compiled VAP module.
#[derive(Debug, Clone)]
enum Expected {
    /// `{"accepted": true}`, with no `message` and no `warnings` field.
    Accepted,
    /// `{"accepted": true, "warnings": [...]}`. Each entry is a substring
    /// that `warnings[i]` must contain. The list lengths must match.
    AcceptedWithWarnings(Vec<&'static str>),
    /// `{"accepted": false, ...}`, optionally asserting `message` and/or
    /// `code`, with no `warnings` field.
    Rejected {
        message: Option<&'static str>,
        code: Option<i32>,
    },
    /// Like [`Expected::Rejected`], with a `warnings` list. Each entry is a
    /// substring that `warnings[i]` must contain. The list lengths must
    /// match.
    RejectedWithWarnings {
        message: Option<&'static str>,
        code: Option<i32>,
        warnings: Vec<&'static str>,
    },
    /// The module traps: `eval()` returns `Err` that downcasts to
    /// [`CelRuntimeError`], whose message contains the given text, and
    /// whose `origin` is `None`.
    Error(&'static str),
    /// Like [`Expected::Error`], but the error must come from the host
    /// extension `namespace.function`.
    ExtensionError {
        message: &'static str,
        namespace: Option<&'static str>,
        function: &'static str,
    },
}

impl Expected {
    /// A rejection asserting both `message` and `code`.
    fn rejected(message: &'static str, code: i32) -> Self {
        Expected::Rejected {
            message: Some(message),
            code: Some(code),
        }
    }

    /// A rejection asserting neither `message` nor `code`.
    fn rejected_any() -> Self {
        Expected::Rejected {
            message: None,
            code: None,
        }
    }

    /// A rejection asserting `message`, with no assertion on `code`.
    fn rejected_message(message: &'static str) -> Self {
        Expected::Rejected {
            message: Some(message),
            code: None,
        }
    }

    /// A rejection asserting `message`, `code`, and the `warnings` list.
    fn rejected_with_warnings(
        message: &'static str,
        code: i32,
        warnings: Vec<&'static str>,
    ) -> Self {
        Expected::RejectedWithWarnings {
            message: Some(message),
            code: Some(code),
            warnings,
        }
    }
}

/// Assert that `result` matches `expected`.
fn assert_outcome(result: Result<serde_json::Value, anyhow::Error>, expected: &Expected) {
    match expected {
        Expected::Accepted => {
            let result = result.expect("expected an accepted response, got an error");
            assert_accepted(&result);
            assert_no_warnings(&result);
        }
        Expected::AcceptedWithWarnings(warnings) => {
            let result = result.expect("expected an accepted response, got an error");
            assert_accepted(&result);
            assert_warnings(&result, warnings);
        }
        Expected::Rejected { message, code } => {
            let result = result.expect("expected a rejected response, got an error");
            assert_rejected(&result, *message, *code);
            assert_no_warnings(&result);
        }
        Expected::RejectedWithWarnings {
            message,
            code,
            warnings,
        } => {
            let result = result.expect("expected a rejected response, got an error");
            assert_rejected(&result, *message, *code);
            assert_warnings(&result, warnings);
        }
        Expected::Error(message) => {
            let cel_err = assert_cel_runtime_error(result, message);
            assert_eq!(
                cel_err.origin, None,
                "expected no extension origin, got: {cel_err:?}"
            );
        }
        Expected::ExtensionError {
            message,
            namespace,
            function,
        } => {
            let cel_err = assert_cel_runtime_error(result, message);
            let expected_origin = ExtensionOrigin {
                namespace: namespace.map(str::to_string),
                function: function.to_string(),
            };
            assert_eq!(
                cel_err.origin.as_ref(),
                Some(&expected_origin),
                "unexpected extension origin in: {cel_err:?}"
            );
        }
    }
}

fn assert_accepted(result: &serde_json::Value) {
    assert_eq!(
        result.get("accepted"),
        Some(&serde_json::Value::Bool(true)),
        "expected accepted=true, got: {result}"
    );
    assert!(
        result.get("message").is_none(),
        "accepted response should have no message, got: {result}"
    );
}

fn assert_rejected(result: &serde_json::Value, message: Option<&str>, code: Option<i32>) {
    assert_eq!(
        result.get("accepted"),
        Some(&serde_json::Value::Bool(false)),
        "expected accepted=false, got: {result}"
    );
    if let Some(expected_msg) = message {
        assert_eq!(
            result.get("message").and_then(|v| v.as_str()),
            Some(expected_msg),
            "unexpected rejection message, got: {result}"
        );
    }
    if let Some(expected_code) = code {
        assert_eq!(
            result.get("code").and_then(|v| v.as_i64()),
            Some(i64::from(expected_code)),
            "unexpected rejection code, got: {result}"
        );
    }
}

/// The `warnings` key must be absent, not an empty list. As a result, a
/// module that skips nothing produces the same JSON as before
/// `failurePolicy` support.
fn assert_no_warnings(result: &serde_json::Value) {
    assert!(
        result.get("warnings").is_none(),
        "response should have no warnings, got: {result}"
    );
}

/// `warnings` must be a list of the same length as `expected`, and
/// `warnings[i]` must contain `expected[i]`.
fn assert_warnings(result: &serde_json::Value, expected: &[&str]) {
    let warnings = result
        .get("warnings")
        .and_then(|w| w.as_array())
        .unwrap_or_else(|| panic!("expected a warnings list, got: {result}"));
    let warnings: Vec<&str> = warnings
        .iter()
        .map(|w| w.as_str().expect("warning is a string"))
        .collect();
    assert_eq!(
        warnings.len(),
        expected.len(),
        "unexpected number of warnings, got: {warnings:?}"
    );
    for (got, want) in warnings.iter().zip(expected) {
        assert!(
            got.contains(want),
            "expected {want:?} in warning, got: {got:?}"
        );
    }
}

/// Assert that `result` is a [`CelRuntimeError`] whose message contains
/// `expected_message`. Return a clone of the error for further checks.
fn assert_cel_runtime_error(
    result: Result<serde_json::Value, anyhow::Error>,
    expected_message: &str,
) -> CelRuntimeError {
    let err = result.expect_err("expected a runtime error, got a response");

    // The `Display` text keeps the legacy prefix.
    let msg = format!("{err:#}");
    assert!(
        msg.contains("CEL runtime error"),
        "expected a CEL runtime error, got: {msg}"
    );
    assert!(
        msg.contains(expected_message),
        "expected {expected_message:?} in error, got: {msg}"
    );

    // The typed error is what a host must use to tell a CEL error apart
    // from other failures.
    let cel_err = err
        .downcast_ref::<CelRuntimeError>()
        .unwrap_or_else(|| panic!("error does not downcast to CelRuntimeError: {err:#}"));
    assert!(
        cel_err.message.contains(expected_message),
        "expected {expected_message:?} in CelRuntimeError.message, got: {cel_err:?}"
    );
    cel_err.clone()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

/// A policy with a single validation that passes → accepted.
#[test]
fn test_vap_accept_simple() {
    let spec = r#"spec:
  validations:
    - expression: "object.spec.replicas <= 5"
      message: "too many replicas"
"#;
    let bindings = serde_json::json!({ "object": { "spec": { "replicas": 3 } } }).to_string();
    assert_outcome(eval_vap(spec, &bindings, None), &Expected::Accepted);
}

/// A policy with a single validation that fails → rejected with static message.
#[test]
fn test_vap_reject_static_message() {
    let spec = r#"spec:
  validations:
    - expression: "object.spec.replicas <= 5"
      message: "too many replicas"
"#;
    let bindings = serde_json::json!({ "object": { "spec": { "replicas": 10 } } }).to_string();
    assert_outcome(
        eval_vap(spec, &bindings, None),
        &Expected::rejected_message("too many replicas"),
    );
}

/// Validation fails and a `messageExpression` is evaluated to build the message.
#[test]
fn test_vap_reject_message_expression() {
    let spec = r#"spec:
  validations:
    - expression: "object.spec.replicas <= 5"
      messageExpression: "'replicas ' + string(object.spec.replicas) + ' exceeds limit 5'"
"#;
    let bindings = serde_json::json!({ "object": { "spec": { "replicas": 7 } } }).to_string();
    assert_outcome(
        eval_vap(spec, &bindings, None),
        &Expected::rejected_message("replicas 7 exceeds limit 5"),
    );
}

/// A matchCondition that evaluates to false → policy skipped → accepted.
#[test]
fn test_vap_match_condition_false_skips_policy() {
    let spec = r#"spec:
  matchConditions:
    - name: only-deployments
      expression: "object.kind == 'Deployment'"
  validations:
    - expression: "object.spec.replicas <= 5"
      message: "too many replicas"
"#;
    // object.kind is "Pod" → matchCondition false → policy skipped → accept
    // even though replicas=99 would otherwise fail.
    let bindings = serde_json::json!({
        "object": { "kind": "Pod", "spec": { "replicas": 99 } }
    })
    .to_string();
    assert_outcome(eval_vap(spec, &bindings, None), &Expected::Accepted);
}

/// A matchCondition that evaluates to true → validation is enforced → rejected.
#[test]
fn test_vap_match_condition_true_enforces_validation() {
    let spec = r#"spec:
  matchConditions:
    - name: only-deployments
      expression: "object.kind == 'Deployment'"
  validations:
    - expression: "object.spec.replicas <= 5"
      message: "too many replicas"
"#;
    let bindings = serde_json::json!({
        "object": { "kind": "Deployment", "spec": { "replicas": 99 } }
    })
    .to_string();
    assert_outcome(eval_vap(spec, &bindings, None), &Expected::rejected_any());
}

/// Variables are evaluated and accessible in validation expressions.
#[rstest]
#[case::within_limit(4, Expected::Accepted)]
#[case::over_limit(10, Expected::rejected_any())]
fn test_vap_variables(#[case] replicas: i64, #[case] expected: Expected) {
    let spec = r#"spec:
  variables:
    - name: maxReplicas
      expression: "5"
  validations:
    - expression: "object.spec.replicas <= variables.maxReplicas"
      message: "too many replicas"
"#;
    let bindings =
        serde_json::json!({ "object": { "spec": { "replicas": replicas } } }).to_string();
    assert_outcome(eval_vap(spec, &bindings, None), &expected);
}

/// Multiple validations: first passes, second fails → rejection with second
/// validation's message.
#[test]
fn test_vap_multiple_validations_second_fails() {
    let spec = r#"spec:
  validations:
    - expression: "object.spec.replicas >= 1"
      message: "must have at least 1 replica"
    - expression: "object.spec.replicas <= 5"
      message: "too many replicas"
"#;
    let bindings = serde_json::json!({ "object": { "spec": { "replicas": 10 } } }).to_string();
    assert_outcome(
        eval_vap(spec, &bindings, None),
        &Expected::rejected_message("too many replicas"),
    );
}

/// Validation with a reason maps to the correct HTTP status code.
#[test]
fn test_vap_reason_to_code() {
    let spec = r#"spec:
  validations:
    - expression: "false"
      message: "forbidden"
      reason: "Forbidden"
"#;
    let bindings = serde_json::json!({}).to_string();
    assert_outcome(
        eval_vap(spec, &bindings, None),
        &Expected::rejected("forbidden", 403),
    );
}

/// All validations pass → accepted, no rejection fields present.
#[test]
fn test_vap_all_validations_pass() {
    let spec = r#"spec:
  validations:
    - expression: "object.spec.replicas >= 1"
      message: "must have at least 1 replica"
    - expression: "object.spec.replicas <= 10"
      message: "too many replicas"
    - expression: "object.metadata.name != ''"
      message: "name must not be empty"
"#;
    let bindings = serde_json::json!({
        "object": {
            "metadata": { "name": "my-deployment" },
            "spec": { "replicas": 3 }
        }
    })
    .to_string();
    assert_outcome(eval_vap(spec, &bindings, None), &Expected::Accepted);
}

// ─── Non-boolean validation results ────────────────────────────────────────
//
// Kubernetes denies a validation whose result is not exactly `true`, not
// only a validation that evaluates to `false`. This matters for a
// validation that reads a dynamically typed field: the field can hold a
// string, a number, `null`, or a collection instead of a boolean. The
// module must treat every one of these results as a rejection, like
// Kubernetes.
//
// A matchCondition works differently: only `false` skips the param, like
// Kubernetes. `test_vap_match_condition_non_boolean_result_matches` below
// checks this.

/// A validation that reads `object.spec.approved`, a dynamically typed
/// field.
const APPROVED_SPEC: &str = r#"spec:
  validations:
    - expression: "object.spec.approved"
      message: "must be approved"
      reason: "Forbidden"
"#;

/// Only `true` accepts the request. Every other value, including `false`,
/// a string, a number, `null`, a list, and a map, rejects it with the
/// validation's message.
///
/// A matchCondition works differently: only `false` skips the param, like
/// Kubernetes. `test_vap_match_condition_non_boolean_result_matches` below
/// checks this.
#[rstest]
#[case::bool_true(serde_json::json!(true), Expected::Accepted)]
#[case::bool_false(serde_json::json!(false), Expected::rejected("must be approved", 403))]
#[case::string_false(serde_json::json!("false"), Expected::rejected("must be approved", 403))]
#[case::string_true(serde_json::json!("true"), Expected::rejected("must be approved", 403))]
#[case::null(serde_json::json!(null), Expected::rejected("must be approved", 403))]
#[case::zero(serde_json::json!(0), Expected::rejected("must be approved", 403))]
#[case::one(serde_json::json!(1), Expected::rejected("must be approved", 403))]
#[case::empty_list(serde_json::json!([]), Expected::rejected("must be approved", 403))]
#[case::empty_map(serde_json::json!({}), Expected::rejected("must be approved", 403))]
fn test_vap_validation_result_only_true_accepts(
    #[case] approved: serde_json::Value,
    #[case] expected: Expected,
) {
    let bindings =
        serde_json::json!({ "object": { "spec": { "approved": approved } } }).to_string();
    assert_outcome(eval_vap(APPROVED_SPEC, &bindings, None), &expected);
}

/// Under `failurePolicy: Ignore`, a non-boolean result is still a
/// rejection, not a skipped expression. Only a CEL runtime error triggers a
/// skip and a warning.
#[test]
fn test_vap_ignore_non_boolean_validation_result_still_rejects() {
    let bindings = bindings_with_failure_policy(
        Some(serde_json::json!("Ignore")),
        serde_json::json!({ "spec": { "approved": "false" } }),
    );
    assert_outcome(
        eval_vap(APPROVED_SPEC, &bindings, None),
        &Expected::rejected("must be approved", 403),
    );
}

/// A `messageExpression` still runs when the validation result is a
/// non-boolean value, and its result still wins over the static message.
#[test]
fn test_vap_non_boolean_validation_result_uses_message_expression() {
    let spec = r#"spec:
  validations:
    - expression: "object.spec.approved"
      messageExpression: "'approval value: ' + string(object.spec.approved)"
      message: "must be approved"
"#;
    let bindings = serde_json::json!({ "object": { "spec": { "approved": "false" } } }).to_string();
    assert_outcome(
        eval_vap(spec, &bindings, None),
        &Expected::rejected_message("approval value: false"),
    );
}

/// A matchCondition that evaluates to a non-boolean result still counts as
/// a match, like Kubernetes. Only `false` skips the param.
#[test]
fn test_vap_match_condition_non_boolean_result_matches() {
    let spec = r#"spec:
  matchConditions:
    - name: has-tier
      expression: "object.spec.tier"
  validations:
    - expression: "false"
      message: "never passes"
"#;
    let bindings = serde_json::json!({ "object": { "spec": { "tier": "gold" } } }).to_string();
    assert_outcome(
        eval_vap(spec, &bindings, None),
        &Expected::rejected("never passes", 422),
    );
}

// ─── Runtime errors ───────────────────────────────────────────────────────────
//
// These tests send no `failurePolicy` binding, so the policy is `Fail`. When
// a matchCondition or validation evaluates to a CEL runtime error, the module
// must trap, and `eval()` must return `Err`. The module must never turn the
// error into an accept or a rejection. The section "failurePolicy: Ignore"
// below covers the `Ignore` behavior.

const EMPTY_OBJECT_BINDINGS: &str = r#"{"object": {}}"#;

/// A validation whose expression errors → `eval()` returns `Err`, not
/// `{"accepted": true}`.
#[test]
fn test_vap_validation_runtime_error_is_surfaced() {
    let spec = r#"spec:
  validations:
    - expression: "(1 / 0) == 1"
      message: "never used"
"#;
    assert_outcome(
        eval_vap(spec, EMPTY_OBJECT_BINDINGS, None),
        &Expected::Error("divide by zero"),
    );
}

/// A validation that reads a missing field errors at runtime. The host gets
/// an error that downcasts to [`CelRuntimeError`], with no extension origin.
#[test]
fn test_vap_missing_field_error_downcasts_to_cel_runtime_error() {
    let spec = r#"spec:
  validations:
    - expression: "object.missing.field > 1"
"#;
    // `object.missing` is a "no such key" error. Field access and `>`
    // propagate it unchanged, so the host sees the original message.
    assert_outcome(
        eval_vap(spec, EMPTY_OBJECT_BINDINGS, None),
        &Expected::Error("no such key: 'missing'"),
    );
}

/// A missing-field error is a value, not an abort. The `||` operator
/// absorbs it like any other CEL runtime error.
#[test]
fn test_vap_missing_field_error_absorbed_by_or_is_accepted() {
    let spec = r#"spec:
  validations:
    - expression: "object.missing.field > 1 || true"
"#;
    assert_outcome(
        eval_vap(spec, EMPTY_OBJECT_BINDINGS, None),
        &Expected::Accepted,
    );
}

/// Under `Fail`, a validation that passes does not hide a later validation
/// that evaluates to an error.
#[test]
fn test_vap_validation_runtime_error_after_passing_validation() {
    let spec = r#"spec:
  validations:
    - expression: "true"
    - expression: "(1 / 0) == 1"
"#;
    assert_outcome(
        eval_vap(spec, EMPTY_OBJECT_BINDINGS, None),
        &Expected::Error("divide by zero"),
    );
}

/// A validation that references an unbound variable errors instead of passing.
#[test]
fn test_vap_validation_unbound_variable_is_surfaced() {
    let spec = r#"spec:
  validations:
    - expression: "doesNotExist == 1"
"#;
    assert_outcome(
        eval_vap(spec, EMPTY_OBJECT_BINDINGS, None),
        &Expected::Error("doesNotExist"),
    );
}

/// CEL short-circuit semantics still apply: an error absorbed by `||` is not
/// an error for the validation.
#[test]
fn test_vap_validation_error_absorbed_by_or_is_accepted() {
    let spec = r#"spec:
  validations:
    - expression: "(1 / 0) == 1 || true"
"#;
    assert_outcome(
        eval_vap(spec, EMPTY_OBJECT_BINDINGS, None),
        &Expected::Accepted,
    );
}

/// A variable that errors is harmless as long as no validation references it
/// (K8s variables are lazy).
#[test]
fn test_vap_unused_erroring_variable_is_harmless() {
    let spec = r#"spec:
  variables:
    - name: broken
      expression: "1 / 0"
  validations:
    - expression: "true"
"#;
    assert_outcome(
        eval_vap(spec, EMPTY_OBJECT_BINDINGS, None),
        &Expected::Accepted,
    );
}

/// A variable that errors propagates the error into the validation that uses it.
#[test]
fn test_vap_used_erroring_variable_is_surfaced() {
    let spec = r#"spec:
  variables:
    - name: broken
      expression: "1 / 0"
  validations:
    - expression: "variables.broken == 1"
"#;
    assert_outcome(
        eval_vap(spec, EMPTY_OBJECT_BINDINGS, None),
        &Expected::Error("divide by zero"),
    );
}

/// A host extension failure inside a validation (e.g. `kw.k8s...get()`
/// returning an error) surfaces as a runtime error, rather than being
/// silently accepted. The error records `kw.k8s.get` as its origin.
#[test]
fn test_vap_extension_error_in_validation_is_surfaced() {
    let spec = r#"spec:
  validations:
    - expression: "kw.k8s.apiVersion('v1').kind('ConfigMap').namespace('default').get('cfg').data.ok == 'true'"
      message: "config must be ok"
"#;
    let result = eval_vap(
        spec,
        EMPTY_OBJECT_BINDINGS,
        Some(failing(vap::kw_k8s_get_extension(), "boom")),
    );
    assert_outcome(
        result,
        &Expected::ExtensionError {
            message: "boom",
            namespace: Some("kw.k8s"),
            function: "get",
        },
    );
}

// ─── failurePolicy: Ignore ────────────────────────────────────────────────────
//
// The host passes `failurePolicy` in the bindings. Under `Ignore`, the module
// applies the policy to each expression on its own, like Kubernetes. When a
// validation evaluates to an error, the module skips it, records a warning,
// and runs the next one. When a matchCondition evaluates to an error, the
// module skips the current param and runs the next param. A `false` result
// still rejects.

/// Bindings with `object` and the given `failurePolicy` value. `None` omits
/// the binding.
fn bindings_with_failure_policy(
    policy: Option<serde_json::Value>,
    object: serde_json::Value,
) -> String {
    let mut bindings = serde_json::json!({ "object": object });
    if let Some(policy) = policy {
        bindings["failurePolicy"] = policy;
    }
    bindings.to_string()
}

/// `bindings_with_failure_policy` with `failurePolicy: Ignore`. Most
/// `Ignore` tests need no other field on `object`.
fn ignore_bindings(object: serde_json::Value) -> String {
    bindings_with_failure_policy(Some("Ignore".into()), object)
}

/// Under `Ignore`, the module skips the validation that evaluates to an
/// error, and the next one decides. Under `Fail` (explicit, missing, or
/// `null`), the module traps.
///
/// `[0]` evaluates to an error because `object.missing` does not exist.
/// `[1]` decides on `object.spec.replicas`.
#[rstest]
#[case::ignore_error_then_reject(
    Some(serde_json::json!("Ignore")),
    100,
    Expected::rejected_with_warnings(
        "too many replicas",
        422,
        vec!["The module skipped validation[0] because the expression evaluated to an error (failurePolicy is Ignore): no such key: 'missing'"],
    )
)]
#[case::ignore_error_then_accept(
    Some(serde_json::json!("Ignore")),
    1,
    Expected::AcceptedWithWarnings(vec!["The module skipped validation[0] because the expression evaluated to an error (failurePolicy is Ignore): no such key: 'missing'"])
)]
#[case::fail_explicit(Some(serde_json::json!("Fail")), 100, Expected::Error("no such key: 'missing'"))]
#[case::fail_missing_binding(None, 100, Expected::Error("no such key: 'missing'"))]
#[case::fail_null_binding(Some(serde_json::json!(null)), 100, Expected::Error("no such key: 'missing'"))]
fn test_vap_failure_policy_error_then_validation(
    #[case] policy: Option<serde_json::Value>,
    #[case] replicas: i64,
    #[case] expected: Expected,
) {
    let spec = r#"spec:
  validations:
    - expression: "object.missing.x > 1"
      message: "never used"
    - expression: "object.spec.replicas <= 3"
      message: "too many replicas"
"#;
    let bindings = bindings_with_failure_policy(
        policy,
        serde_json::json!({ "spec": { "replicas": replicas } }),
    );
    assert_outcome(eval_vap(spec, &bindings, None), &expected);
}

/// The motivating example: `int()` on a non-numeric annotation used to trap
/// the whole policy even under `Ignore`, because `cel_int` aborted instead
/// of returning an error value. It now evaluates to an error value like any
/// other CEL runtime error, so `Ignore` skips it and the next validation
/// decides.
#[rstest]
#[case::ignore_bad_annotation_second_denies(
    Some(serde_json::json!("Ignore")),
    "invalid",
    100,
    Expected::rejected_with_warnings(
        "too many replicas",
        422,
        vec!["validation[0]"],
    )
)]
#[case::ignore_bad_annotation_second_passes(
    Some(serde_json::json!("Ignore")),
    "invalid",
    1,
    Expected::AcceptedWithWarnings(vec!["validation[0]"])
)]
#[case::fail_bad_annotation(
    Some(serde_json::json!("Fail")),
    "invalid",
    100,
    Expected::Error("type conversion error from 'string' to 'int'")
)]
fn test_vap_failure_policy_int_conversion_error(
    #[case] policy: Option<serde_json::Value>,
    #[case] count: &str,
    #[case] replicas: i64,
    #[case] expected: Expected,
) {
    let spec = r#"spec:
  validations:
    - expression: "int(object.metadata.annotations['count']) <= 10"
      message: "count too high"
    - expression: "object.spec.replicas <= 3"
      message: "too many replicas"
"#;
    let bindings = bindings_with_failure_policy(
        policy,
        serde_json::json!({
            "metadata": { "annotations": { "count": count } },
            "spec": { "replicas": replicas }
        }),
    );
    assert_outcome(eval_vap(spec, &bindings, None), &expected);
}

/// The label names the validation that evaluated to an error, not the first
/// one.
#[test]
fn test_vap_ignore_true_then_error_is_accepted_with_warning() {
    let spec = r#"spec:
  validations:
    - expression: "true"
    - expression: "(1 / 0) == 1"
"#;
    let bindings = ignore_bindings(serde_json::json!({}));
    assert_outcome(
        eval_vap(spec, &bindings, None),
        &Expected::AcceptedWithWarnings(vec![
            "The module skipped validation[1] because the expression evaluated to an error (failurePolicy is Ignore): divide by zero",
        ]),
    );
}

/// Each validation that evaluates to an error adds one warning. The order is
/// the evaluation order.
#[test]
fn test_vap_ignore_two_errors_give_two_warnings_in_order() {
    let spec = r#"spec:
  validations:
    - expression: "(1 / 0) == 1"
    - expression: "object.missing.x > 1"
"#;
    let bindings = ignore_bindings(serde_json::json!({}));
    assert_outcome(
        eval_vap(spec, &bindings, None),
        &Expected::AcceptedWithWarnings(vec![
            "The module skipped validation[0] because the expression evaluated to an error (failurePolicy is Ignore): divide by zero",
            "The module skipped validation[1] because the expression evaluated to an error (failurePolicy is Ignore): no such key: 'missing'",
        ]),
    );
}

/// The first `false` returns the rejection. The validation after it never
/// runs, so there is no warning. Kubernetes runs `[1]` too, but the decision
/// (deny) is the same.
#[test]
fn test_vap_ignore_false_then_error_rejects_without_warnings() {
    let spec = r#"spec:
  validations:
    - expression: "false"
      message: "first failed"
    - expression: "(1 / 0) == 1"
"#;
    let bindings = ignore_bindings(serde_json::json!({}));
    assert_outcome(
        eval_vap(spec, &bindings, None),
        &Expected::rejected("first failed", 422),
    );
}

/// If the `failurePolicy` value is invalid, the module traps before the
/// first expression runs. The value is case-sensitive, like the Kubernetes
/// field.
#[rstest]
#[case::lowercase(serde_json::json!("ignore"))]
#[case::bool(serde_json::json!(true))]
#[case::int(serde_json::json!(1))]
fn test_vap_invalid_failure_policy_is_error(#[case] policy: serde_json::Value) {
    let spec = r#"spec:
  validations:
    - expression: "true"
"#;
    let bindings = bindings_with_failure_policy(Some(policy), serde_json::json!({}));
    assert_outcome(
        eval_vap(spec, &bindings, None),
        &Expected::Error("failurePolicy"),
    );
}

/// Build a VAP `spec:` with the given `matchConditions` (name, expression
/// pairs) and one validation that always fails. A test that checks how
/// `matchConditions` interact with `failurePolicy` uses this so that a
/// skipped policy (accepted) is distinguishable from an enforced one
/// (rejected).
fn match_conditions_spec(conditions: &[(&str, &str)]) -> String {
    let mut spec = String::from("spec:\n  matchConditions:\n");
    for (name, expression) in conditions {
        spec.push_str(&format!(
            "    - name: {name}\n      expression: \"{expression}\"\n"
        ));
    }
    spec.push_str(
        "  validations:\n    - expression: \"false\"\n      message: \"never reached\"\n",
    );
    spec
}

/// How `matchConditions` interact with `failurePolicy`, across the number
/// of conditions and their outcomes (`true`, `false`, error). A `false`
/// result always wins over an error in another condition of the same
/// param, whatever the order. `failurePolicy` decides the outcome of an
/// error only when no condition is `false`. Under `Ignore`, the module
/// stops at the first errored condition, so there is at most one warning
/// per param.
#[rstest]
#[case::none_binding_error_traps(
    None,
    &[("broken", "(1 / 0) == 1")][..],
    Expected::Error("divide by zero"),
)]
#[case::fail_error_traps(
    Some("Fail"),
    &[("broken", "(1 / 0) == 1")][..],
    Expected::Error("divide by zero"),
)]
#[case::ignore_error_skips_policy(
    Some("Ignore"),
    &[("broken", "(1 / 0) == 1")][..],
    Expected::AcceptedWithWarnings(vec![
        "The module skipped the policy because matchCondition 'broken' evaluated to an error (failurePolicy is Ignore): divide by zero",
    ]),
)]
#[case::ignore_two_errors_stop_at_first(
    Some("Ignore"),
    &[("broken", "(1 / 0) == 1"), ("also-broken", "object.missing.x > 1")][..],
    Expected::AcceptedWithWarnings(vec!["matchCondition 'broken'"]),
)]
#[case::fail_false_wins_over_earlier_error(
    Some("Fail"),
    &[("broken", "(1 / 0) == 1"), ("excluded", "false")][..],
    Expected::Accepted,
)]
#[case::fail_false_wins_over_later_error(
    Some("Fail"),
    &[("excluded", "false"), ("broken", "(1 / 0) == 1")][..],
    Expected::Accepted,
)]
#[case::ignore_false_wins_over_error_no_warning(
    Some("Ignore"),
    &[("broken", "(1 / 0) == 1"), ("excluded", "false")][..],
    Expected::Accepted,
)]
#[case::fail_true_then_error_still_traps(
    Some("Fail"),
    &[("passes", "true"), ("broken", "(1 / 0) == 1")][..],
    Expected::Error("divide by zero"),
)]
#[case::ignore_true_then_error_one_warning(
    Some("Ignore"),
    &[("passes", "true"), ("broken", "(1 / 0) == 1")][..],
    Expected::AcceptedWithWarnings(vec!["matchCondition 'broken'"]),
)]
fn test_vap_match_conditions_false_wins_over_error(
    #[case] policy: Option<&str>,
    #[case] conditions: &[(&str, &str)],
    #[case] expected: Expected,
) {
    let spec = match_conditions_spec(conditions);
    let bindings =
        bindings_with_failure_policy(policy.map(|p| serde_json::json!(p)), serde_json::json!({}));
    assert_outcome(eval_vap(&spec, &bindings, None), &expected);
}

/// A host extension error inside a validation is a CEL runtime error like
/// any other. Under `Ignore`, the module skips the validation, and the
/// warning carries the host message.
#[test]
fn test_vap_ignore_extension_error_in_validation_is_skipped() {
    let spec = r#"spec:
  validations:
    - expression: "kw.k8s.apiVersion('v1').kind('ConfigMap').namespace('default').get('cfg').data.ok == 'true'"
      message: "config must be ok"
"#;
    let bindings = ignore_bindings(serde_json::json!({}));
    let result = eval_vap(
        spec,
        &bindings,
        Some(failing(vap::kw_k8s_get_extension(), "boom")),
    );
    assert_outcome(
        result,
        &Expected::AcceptedWithWarnings(vec![
            "The module skipped validation[0] because the expression evaluated to an error (failurePolicy is Ignore): boom",
        ]),
    );
}

/// `Ignore` does not cover the `params` lookup. A host error there traps,
/// with its `kw.k8s.list` origin, so the host can apply the policy.
#[test]
fn test_vap_ignore_params_lookup_error_still_traps() {
    let result = eval_vap(
        PARAMS_SPEC,
        &params_bindings_ignore(selector_ref(None), 3),
        Some(failing(vap::kw_k8s_list_extension(), "forbidden")),
    );
    assert_outcome(
        result,
        &Expected::ExtensionError {
            message: "forbidden",
            namespace: Some("kw.k8s"),
            function: "list",
        },
    );
}

/// `params_bindings` plus `failurePolicy: Ignore`.
fn params_bindings_ignore(param_ref: serde_json::Value, replicas: i64) -> String {
    let mut bindings: serde_json::Value =
        serde_json::from_str(&params_bindings(param_ref, replicas)).unwrap();
    bindings["failurePolicy"] = "Ignore".into();
    bindings.to_string()
}

/// Two params from a selector. The first has no `maxReplicas`, so the
/// validation evaluates to `no such key` for it. Under `Ignore`, the module
/// skips that validation, records a warning, and the second param decides.
/// The warning names the param.
#[rstest]
#[case::second_denies(
    "1",
    Expected::rejected_with_warnings(
        "limit 1 from second",
        422,
        vec!["The module skipped validation[0] for params default/first because the expression evaluated to an error (failurePolicy is Ignore): no such key: 'maxReplicas'"],
    )
)]
#[case::second_passes(
    "10",
    Expected::AcceptedWithWarnings(vec![
        "The module skipped validation[0] for params default/first because the expression evaluated to an error (failurePolicy is Ignore): no such key: 'maxReplicas'",
    ])
)]
fn test_vap_ignore_params_first_errors_second_decides(
    #[case] second_max: &str,
    #[case] expected: Expected,
) {
    let mut first = params_configmap("first", "1");
    first["data"] = serde_json::json!({});
    let items = vec![first, params_configmap("second", second_max)];
    let result = eval_vap(
        PARAMS_SPEC,
        &params_bindings_ignore(selector_ref(None), 5),
        Some(list_returning(items)),
    );
    assert_outcome(result, &expected);
}

/// Build a VAP `spec:` with `paramKind: v1/ConfigMap`, the given
/// `matchConditions` (name, expression pairs), and a validation against
/// `params.data.maxReplicas`. Used by tests that check how a per-param
/// `matchCondition` interacts with `failurePolicy` under `paramKind`.
fn params_match_conditions_spec(conditions: &[(&str, &str)]) -> String {
    let mut spec = String::from(
        "spec:\n  paramKind:\n    apiVersion: v1\n    kind: ConfigMap\n  matchConditions:\n",
    );
    for (name, expression) in conditions {
        spec.push_str(&format!(
            "    - name: {name}\n      expression: \"{expression}\"\n"
        ));
    }
    spec.push_str(concat!(
        "  validations:\n",
        "    - expression: \"object.spec.replicas <= int(params.data.maxReplicas)\"\n",
        "      messageExpression: \"'limit ' + params.data.maxReplicas + ' from ' + params.metadata.name\"\n",
    ));
    spec
}

/// A ConfigMap param with `maxReplicas`, and `data.enabled` only when
/// `enabled` is `Some`.
fn params_configmap_maybe_enabled(
    name: &str,
    max_replicas: &str,
    enabled: Option<bool>,
) -> serde_json::Value {
    let mut cm = params_configmap(name, max_replicas);
    if let Some(enabled) = enabled {
        cm["data"]["enabled"] = enabled.into();
    }
    cm
}

/// A matchCondition that reads `params`, with two params from a selector.
/// The first param has no `enabled` key, so the condition evaluates to an
/// error for it. Under `Ignore`, the module skips that param, and the
/// second param decides. The warning names the skipped param.
///
/// The third case adds a second condition to the first param: a `false`
/// result that wins over the `enabled` error, so the module records no
/// warning for it. This holds inside the `paramKind` loop, not only for a
/// single param.
#[rstest]
#[case::second_denies(
    &[("enabled", "params.data.enabled == true")][..],
    "1",
    Expected::rejected_with_warnings(
        "limit 1 from second",
        422,
        vec![
            "The module skipped params default/first because matchCondition 'enabled' evaluated to an error (failurePolicy is Ignore): no such key: 'enabled'",
        ],
    )
)]
#[case::second_passes(
    &[("enabled", "params.data.enabled == true")][..],
    "10",
    Expected::AcceptedWithWarnings(vec![
        "The module skipped params default/first because matchCondition 'enabled' evaluated to an error (failurePolicy is Ignore): no such key: 'enabled'",
    ])
)]
#[case::false_condition_wins_over_error_no_warning(
    &[
        ("enabled", "params.data.enabled == true"),
        ("not-first", "params.metadata.name != 'first'"),
    ][..],
    "10",
    Expected::Accepted,
)]
fn test_vap_ignore_params_first_match_condition_errors_second_decides(
    #[case] conditions: &[(&str, &str)],
    #[case] second_max: &str,
    #[case] expected: Expected,
) {
    let spec = params_match_conditions_spec(conditions);
    let items = vec![
        params_configmap_maybe_enabled("first", "1", None),
        params_configmap_maybe_enabled("second", second_max, Some(true)),
    ];
    let result = eval_vap(
        &spec,
        &params_bindings_ignore(selector_ref(None), 5),
        Some(list_returning(items)),
    );
    assert_outcome(result, &expected);
}

/// The warnings belong to one evaluation. A host uses one `Engine` for many
/// requests, so the second evaluation must not see the warnings of the
/// first.
#[test]
fn test_vap_ignore_warnings_reset_between_evaluations() {
    let spec = r#"spec:
  validations:
    - expression: "object.a.x > 1"
    - expression: "object.b.x > 1"
"#;
    // First: both keys missing → two warnings. Second: only `b` missing → one
    // warning, for `[1]` only. Third: nothing missing → no warnings key.
    let first = ignore_bindings(serde_json::json!({}));
    let second = bindings_with_failure_policy(
        Some("Ignore".into()),
        serde_json::json!({ "a": { "x": 2 } }),
    );
    let third = bindings_with_failure_policy(
        Some("Ignore".into()),
        serde_json::json!({ "a": { "x": 2 }, "b": { "x": 2 } }),
    );

    let results = eval_vap_repeatedly(spec, &[&first, &second, &third], vec![]).unwrap();
    let mut results = results.into_iter();

    assert_outcome(
        results.next().unwrap(),
        &Expected::AcceptedWithWarnings(vec!["validation[0]", "validation[1]"]),
    );
    assert_outcome(
        results.next().unwrap(),
        &Expected::AcceptedWithWarnings(vec![
            "The module skipped validation[1] because the expression evaluated to an error (failurePolicy is Ignore): no such key: 'b'",
        ]),
    );
    assert_outcome(results.next().unwrap(), &Expected::Accepted);
}

/// The module reads `failurePolicy` on every evaluation, not only on the
/// first.
#[test]
fn test_vap_failure_policy_reread_between_evaluations() {
    let spec = r#"spec:
  validations:
    - expression: "(1 / 0) == 1"
"#;
    let ignore = ignore_bindings(serde_json::json!({}));
    let fail = bindings_with_failure_policy(Some("Fail".into()), serde_json::json!({}));

    let results = eval_vap_repeatedly(spec, &[&ignore, &fail, &ignore], vec![]).unwrap();
    let mut results = results.into_iter();

    assert_outcome(
        results.next().unwrap(),
        &Expected::AcceptedWithWarnings(vec!["validation[0]"]),
    );
    assert_outcome(results.next().unwrap(), &Expected::Error("divide by zero"));
    assert_outcome(
        results.next().unwrap(),
        &Expected::AcceptedWithWarnings(vec!["validation[0]"]),
    );
}

// ─── messageExpression fallback ───────────────────────────────────────────────

/// A `messageExpression` that errors falls back to the static `message`.
#[test]
fn test_vap_message_expression_error_falls_back_to_static_message() {
    let spec = r#"spec:
  validations:
    - expression: "false"
      message: "static message"
      messageExpression: "string(1 / 0)"
"#;
    assert_outcome(
        eval_vap(spec, EMPTY_OBJECT_BINDINGS, None),
        &Expected::rejected("static message", 422),
    );
}

/// A `messageExpression` that errors, with no static `message`, falls back to
/// the default message derived from the expression text.
#[test]
fn test_vap_message_expression_error_falls_back_to_default_message() {
    let spec = r#"spec:
  validations:
    - expression: "false"
      messageExpression: "string(1 / 0)"
"#;
    assert_outcome(
        eval_vap(spec, EMPTY_OBJECT_BINDINGS, None),
        &Expected::rejected("failed expression: false", 422),
    );
}

/// A `messageExpression` that produces a non-string value falls back to the
/// static `message`.
#[test]
fn test_vap_message_expression_non_string_falls_back_to_static_message() {
    let spec = r#"spec:
  validations:
    - expression: "false"
      message: "static message"
      messageExpression: "42"
"#;
    assert_outcome(
        eval_vap(spec, EMPTY_OBJECT_BINDINGS, None),
        &Expected::rejected("static message", 422),
    );
}

// ─── no-default-sa-rolebinding ────────────────────────────────────────────────

#[rstest]
#[case::non_default_sa(
    serde_json::json!({
        "object": {
            "subjects": [
                { "kind": "ServiceAccount", "name": "my-service-account", "namespace": "default" }
            ]
        }
    }),
    Expected::Accepted
)]
#[case::no_subjects_field(serde_json::json!({ "object": {} }), Expected::Accepted)]
#[case::default_sa_subject(
    serde_json::json!({
        "object": {
            "subjects": [
                { "kind": "ServiceAccount", "name": "default", "namespace": "default" }
            ]
        }
    }),
    Expected::rejected("subjects cannot include the 'default' service account", 422)
)]
#[case::mixed_subjects_with_default_sa(
    serde_json::json!({
        "object": {
            "subjects": [
                { "kind": "ServiceAccount", "name": "my-sa", "namespace": "default" },
                { "kind": "ServiceAccount", "name": "default", "namespace": "kube-system" },
                { "kind": "User", "name": "alice" }
            ]
        }
    }),
    Expected::rejected("subjects cannot include the 'default' service account", 422)
)]
fn test_vap_no_default_sa_rolebinding(
    #[case] object: serde_json::Value,
    #[case] expected: Expected,
) {
    let spec = r#"spec:
  failurePolicy: Fail
  validations:
    - expression: "!has(object.subjects) || object.subjects.all(s, !(s.kind == 'ServiceAccount' && s.name == 'default'))"
      message: "subjects cannot include the 'default' service account"
      reason: Invalid
"#;
    assert_outcome(eval_vap(spec, &object.to_string(), None), &expected);
}

// ─── pss-privilege-escalation ─────────────────────────────────────────────────

#[rstest]
#[case::pod_all_containers_compliant(
    serde_json::json!({
        "object": {
            "kind": "Pod",
            "spec": {
                "containers": [{
                    "name": "app",
                    "securityContext": { "allowPrivilegeEscalation": false }
                }]
            }
        }
    }),
    Expected::Accepted
)]
#[case::pod_init_and_main_containers_compliant(
    serde_json::json!({
        "object": {
            "kind": "Pod",
            "spec": {
                "initContainers": [{
                    "name": "init",
                    "securityContext": { "allowPrivilegeEscalation": false }
                }],
                "containers": [
                    { "name": "app",     "securityContext": { "allowPrivilegeEscalation": false } },
                    { "name": "sidecar", "securityContext": { "allowPrivilegeEscalation": false } }
                ]
            }
        }
    }),
    Expected::Accepted
)]
#[case::non_pod_kind_skips_all_validations(
    serde_json::json!({ "object": { "kind": "ConfigMap" } }),
    Expected::Accepted
)]
#[case::pod_container_missing_field(
    serde_json::json!({
        "object": {
            "kind": "Pod",
            "spec": {
                "containers": [{ "name": "app", "securityContext": {} }]
            }
        }
    }),
    Expected::rejected(
        "securityContext.allowPrivilegeEscalation must be set to false on any containers, initContainers, and ephemeralContainers in Pods",
        422
    )
)]
#[case::pod_container_set_to_true(
    serde_json::json!({
        "object": {
            "kind": "Pod",
            "spec": {
                "containers": [{
                    "name": "app",
                    "securityContext": { "allowPrivilegeEscalation": true }
                }]
            }
        }
    }),
    Expected::rejected(
        "securityContext.allowPrivilegeEscalation must be set to false on any containers, initContainers, and ephemeralContainers in Pods",
        422
    )
)]
#[case::pod_init_container_violates(
    serde_json::json!({
        "object": {
            "kind": "Pod",
            "spec": {
                "initContainers": [{
                    "name": "init",
                    "securityContext": { "allowPrivilegeEscalation": true }
                }],
                "containers": [{
                    "name": "app",
                    "securityContext": { "allowPrivilegeEscalation": false }
                }]
            }
        }
    }),
    Expected::rejected(
        "securityContext.allowPrivilegeEscalation must be set to false on any containers, initContainers, and ephemeralContainers in Pods",
        422
    )
)]
fn test_vap_pss_privilege_escalation(
    #[case] object: serde_json::Value,
    #[case] expected: Expected,
) {
    let spec = r#"spec:
  failurePolicy: Fail
  validations:
    - expression: "object.kind != 'Pod' ||
        (!has(object.spec.initContainers) || object.spec.initContainers.all(container, has(container.securityContext) && has(container.securityContext.allowPrivilegeEscalation) && container.securityContext.allowPrivilegeEscalation == false)) &&
        (!has(object.spec.ephemeralContainers) || object.spec.ephemeralContainers.all(container, has(container.securityContext) && has(container.securityContext.allowPrivilegeEscalation) && container.securityContext.allowPrivilegeEscalation == false)) &&
        (object.spec.containers.all(container, has(container.securityContext) && has(container.securityContext.allowPrivilegeEscalation) && container.securityContext.allowPrivilegeEscalation == false))"
      message: "securityContext.allowPrivilegeEscalation must be set to false on any containers, initContainers, and ephemeralContainers in Pods"
      reason: Invalid
    - expression: "['Deployment','ReplicaSet','DaemonSet','StatefulSet','Job','ReplicationController'].all(kind, object.kind != kind) ||
        (!has(object.spec.template.spec.initContainers) || object.spec.template.spec.initContainers.all(container, has(container.securityContext) && has(container.securityContext.allowPrivilegeEscalation) && container.securityContext.allowPrivilegeEscalation == false)) &&
        (!has(object.spec.template.spec.ephemeralContainers) || object.spec.template.spec.ephemeralContainers.all(container, has(container.securityContext) && has(container.securityContext.allowPrivilegeEscalation) && container.securityContext.allowPrivilegeEscalation == false)) &&
        (object.spec.template.spec.containers.all(container, has(container.securityContext) && has(container.securityContext.allowPrivilegeEscalation) && container.securityContext.allowPrivilegeEscalation == false))"
      message: "securityContext.allowPrivilegeEscalation must be set to false on containers in Workloads"
      reason: Invalid
    - expression: "object.kind != 'CronJob' ||
        (!has(object.spec.jobTemplate.spec.template.spec.initContainers) || object.spec.jobTemplate.spec.template.spec.initContainers.all(container, has(container.securityContext) && has(container.securityContext.allowPrivilegeEscalation) && container.securityContext.allowPrivilegeEscalation == false)) &&
        (!has(object.spec.jobTemplate.spec.template.spec.ephemeralContainers) || object.spec.jobTemplate.spec.template.spec.ephemeralContainers.all(container, has(container.securityContext) && has(container.securityContext.allowPrivilegeEscalation) && container.securityContext.allowPrivilegeEscalation == false)) &&
        (object.spec.jobTemplate.spec.template.spec.containers.all(container, has(container.securityContext) && has(container.securityContext.allowPrivilegeEscalation) && container.securityContext.allowPrivilegeEscalation == false))"
      message: "securityContext.allowPrivilegeEscalation must be set to false on containers in CronJobs"
      reason: Invalid
    - expression: "object.kind != 'PodTemplate' ||
        (!has(object.template.spec.initContainers) || object.template.spec.initContainers.all(container, has(container.securityContext) && has(container.securityContext.allowPrivilegeEscalation) && container.securityContext.allowPrivilegeEscalation == false)) &&
        (!has(object.template.spec.ephemeralContainers) || object.template.spec.ephemeralContainers.all(container, has(container.securityContext) && has(container.securityContext.allowPrivilegeEscalation) && container.securityContext.allowPrivilegeEscalation == false)) &&
        (object.template.spec.containers.all(container, has(container.securityContext) && has(container.securityContext.allowPrivilegeEscalation) && container.securityContext.allowPrivilegeEscalation == false))"
      message: "securityContext.allowPrivilegeEscalation must be set to false on containers in PodTemplates"
      reason: Invalid
"#;
    assert_outcome(eval_vap(spec, &object.to_string(), None), &expected);
}

// ─── pss-capabilities ─────────────────────────────────────────────────────────

#[rstest]
#[case::drop_all_no_add(
    serde_json::json!({
        "object": {
            "kind": "Pod",
            "spec": {
                "containers": [{
                    "name": "app",
                    "securityContext": { "capabilities": { "drop": ["ALL"] } }
                }]
            }
        }
    }),
    Expected::Accepted
)]
#[case::drop_all_add_net_bind_service(
    serde_json::json!({
        "object": {
            "kind": "Pod",
            "spec": {
                "containers": [{
                    "name": "app",
                    "securityContext": {
                        "capabilities": { "drop": ["ALL"], "add": ["NET_BIND_SERVICE"] }
                    }
                }]
            }
        }
    }),
    Expected::Accepted
)]
#[case::drop_missing_all(
    serde_json::json!({
        "object": {
            "kind": "Pod",
            "spec": {
                "containers": [{
                    "name": "app",
                    "securityContext": { "capabilities": { "drop": ["NET_ADMIN"] } }
                }]
            }
        }
    }),
    Expected::rejected(
        "securityContext.capabilities.drop must include ALL and securityContext.capabilities.add can only include NET_BIND_SERVICE on containers in Pods",
        422
    )
)]
#[case::add_disallowed_capability(
    serde_json::json!({
        "object": {
            "kind": "Pod",
            "spec": {
                "containers": [{
                    "name": "app",
                    "securityContext": {
                        "capabilities": { "drop": ["ALL"], "add": ["SYS_ADMIN"] }
                    }
                }]
            }
        }
    }),
    Expected::rejected(
        "securityContext.capabilities.drop must include ALL and securityContext.capabilities.add can only include NET_BIND_SERVICE on containers in Pods",
        422
    )
)]
#[case::no_security_context(
    serde_json::json!({
        "object": {
            "kind": "Pod",
            "spec": { "containers": [{ "name": "app" }] }
        }
    }),
    Expected::rejected(
        "securityContext.capabilities.drop must include ALL and securityContext.capabilities.add can only include NET_BIND_SERVICE on containers in Pods",
        422
    )
)]
fn test_vap_pss_capabilities(#[case] object: serde_json::Value, #[case] expected: Expected) {
    let spec = r#"spec:
  failurePolicy: Fail
  validations:
    - expression: "object.kind != 'Pod' ||
        (!has(object.spec.initContainers) || object.spec.initContainers.all(container, has(container.securityContext) && has(container.securityContext.capabilities.drop) && ('ALL' in container.securityContext.capabilities.drop) && (!has(container.securityContext.capabilities.add) || (size(container.securityContext.capabilities.add) == 1 && 'NET_BIND_SERVICE' in container.securityContext.capabilities.add)))) &&
        (!has(object.spec.ephemeralContainers) || object.spec.ephemeralContainers.all(container, has(container.securityContext) && has(container.securityContext.capabilities.drop) && ('ALL' in container.securityContext.capabilities.drop) && (!has(container.securityContext.capabilities.add) || (size(container.securityContext.capabilities.add) == 1 && 'NET_BIND_SERVICE' in container.securityContext.capabilities.add)))) &&
        (object.spec.containers.all(container, has(container.securityContext) && has(container.securityContext.capabilities.drop) && ('ALL' in container.securityContext.capabilities.drop) && (!has(container.securityContext.capabilities.add) || (size(container.securityContext.capabilities.add) == 1 && 'NET_BIND_SERVICE' in container.securityContext.capabilities.add))))"
      message: "securityContext.capabilities.drop must include ALL and securityContext.capabilities.add can only include NET_BIND_SERVICE on containers in Pods"
      reason: Invalid
    - expression: "['Deployment','ReplicaSet','DaemonSet','StatefulSet','Job','ReplicationController'].all(kind, object.kind != kind) ||
        (!has(object.spec.template.spec.initContainers) || object.spec.template.spec.initContainers.all(container, has(container.securityContext) && has(container.securityContext.capabilities.drop) && ('ALL' in container.securityContext.capabilities.drop) && (!has(container.securityContext.capabilities.add) || (size(container.securityContext.capabilities.add) == 1 && 'NET_BIND_SERVICE' in container.securityContext.capabilities.add)))) &&
        (!has(object.spec.template.spec.ephemeralContainers) || object.spec.template.spec.ephemeralContainers.all(container, has(container.securityContext) && has(container.securityContext.capabilities.drop) && ('ALL' in container.securityContext.capabilities.drop) && (!has(container.securityContext.capabilities.add) || (size(container.securityContext.capabilities.add) == 1 && 'NET_BIND_SERVICE' in container.securityContext.capabilities.add)))) &&
        (object.spec.template.spec.containers.all(container, has(container.securityContext) && has(container.securityContext.capabilities.drop) && ('ALL' in container.securityContext.capabilities.drop) && (!has(container.securityContext.capabilities.add) || (size(container.securityContext.capabilities.add) == 1 && 'NET_BIND_SERVICE' in container.securityContext.capabilities.add))))"
      message: "securityContext.capabilities.drop must include ALL and securityContext.capabilities.add can only include NET_BIND_SERVICE on containers in Workloads"
      reason: Invalid
    - expression: "object.kind != 'CronJob' ||
        (!has(object.spec.jobTemplate.spec.template.spec.initContainers) || object.spec.jobTemplate.spec.template.spec.initContainers.all(container, has(container.securityContext) && has(container.securityContext.capabilities.drop) && ('ALL' in container.securityContext.capabilities.drop) && (!has(container.securityContext.capabilities.add) || (size(container.securityContext.capabilities.add) == 1 && 'NET_BIND_SERVICE' in container.securityContext.capabilities.add)))) &&
        (!has(object.spec.jobTemplate.spec.template.spec.ephemeralContainers) || object.spec.jobTemplate.spec.template.spec.ephemeralContainers.all(container, has(container.securityContext) && has(container.securityContext.capabilities.drop) && ('ALL' in container.securityContext.capabilities.drop) && (!has(container.securityContext.capabilities.add) || (size(container.securityContext.capabilities.add) == 1 && 'NET_BIND_SERVICE' in container.securityContext.capabilities.add)))) &&
        (object.spec.jobTemplate.spec.template.spec.containers.all(container, has(container.securityContext) && has(container.securityContext.capabilities.drop) && ('ALL' in container.securityContext.capabilities.drop) && (!has(container.securityContext.capabilities.add) || (size(container.securityContext.capabilities.add) == 1 && 'NET_BIND_SERVICE' in container.securityContext.capabilities.add))))"
      message: "securityContext.capabilities.drop must include ALL and securityContext.capabilities.add can only include NET_BIND_SERVICE on containers in CronJobs"
      reason: Invalid
    - expression: "object.kind != 'PodTemplate' ||
        (!has(object.template.spec.initContainers) || object.template.spec.initContainers.all(container, has(container.securityContext) && has(container.securityContext.capabilities.drop) && ('ALL' in container.securityContext.capabilities.drop) && (!has(container.securityContext.capabilities.add) || (size(container.securityContext.capabilities.add) == 1 && 'NET_BIND_SERVICE' in container.securityContext.capabilities.add)))) &&
        (!has(object.template.spec.ephemeralContainers) || object.template.spec.ephemeralContainers.all(container, has(container.securityContext) && has(container.securityContext.capabilities.drop) && ('ALL' in container.securityContext.capabilities.drop) && (!has(container.securityContext.capabilities.add) || (size(container.securityContext.capabilities.add) == 1 && 'NET_BIND_SERVICE' in container.securityContext.capabilities.add)))) &&
        (object.template.spec.containers.all(container, has(container.securityContext) && has(container.securityContext.capabilities.drop) && ('ALL' in container.securityContext.capabilities.drop) && (!has(container.securityContext.capabilities.add) || (size(container.securityContext.capabilities.add) == 1 && 'NET_BIND_SERVICE' in container.securityContext.capabilities.add))))"
      message: "securityContext.capabilities.drop must include ALL and securityContext.capabilities.add can only include NET_BIND_SERVICE on containers in PodTemplates"
      reason: Invalid
"#;
    assert_outcome(eval_vap(spec, &object.to_string(), None), &expected);
}

// ─── params ───────────────────────────────────────────────────────────────────
//
// These tests cover the `params` resolution done by the runtime:
// `paramRef.name` vs `paramRef.selector`, `parameterNotFoundAction`,
// namespace defaulting, and per-param evaluation.

/// The policy used by the params tests. The validation reads
/// `params.data.maxReplicas`. The message names the param, so a test can
/// tell which param produced the rejection.
const PARAMS_SPEC: &str = r#"spec:
  paramKind:
    apiVersion: v1
    kind: ConfigMap
  validations:
    - expression: "object.spec.replicas <= int(params.data.maxReplicas)"
      messageExpression: "'limit ' + params.data.maxReplicas + ' from ' + params.metadata.name"
"#;

/// A ConfigMap param object with the given `maxReplicas`.
fn params_configmap(name: &str, max_replicas: &str) -> serde_json::Value {
    serde_json::json!({
        "apiVersion": "v1",
        "kind": "ConfigMap",
        "metadata": { "name": name, "namespace": "default" },
        "data": { "maxReplicas": max_replicas }
    })
}

/// Bindings with a Deployment that has `replicas` and the given `paramRef`.
/// `request.namespace` is `team-a`.
fn params_bindings(param_ref: serde_json::Value, replicas: i64) -> String {
    serde_json::json!({
        "paramRef": param_ref,
        "request": { "namespace": "team-a" },
        "object": {
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": { "name": "my-app", "namespace": "team-a" },
            "spec": { "replicas": replicas }
        }
    })
    .to_string()
}

/// A `paramRef` with `name`, and an optional `parameterNotFoundAction`.
fn name_ref(action: Option<&str>) -> serde_json::Value {
    let mut param_ref = serde_json::json!({ "name": "replica-policy" });
    if let Some(action) = action {
        param_ref["parameterNotFoundAction"] = action.into();
    }
    param_ref
}

/// A `paramRef` with a `matchLabels` selector, and an optional
/// `parameterNotFoundAction`.
fn selector_ref(action: Option<&str>) -> serde_json::Value {
    let mut param_ref = serde_json::json!({ "selector": { "matchLabels": { "env": "test" } } });
    if let Some(action) = action {
        param_ref["parameterNotFoundAction"] = action.into();
    }
    param_ref
}

/// A `list` host implementation that returns the given items.
fn list_returning(items: Vec<serde_json::Value>) -> (ExtensionDecl, HostFn) {
    (
        vap::kw_k8s_list_extension(),
        Box::new(move |_args| Ok(serde_json::json!({ "items": items }))),
    )
}

/// A host implementation that fails with `message`.
fn failing(decl: ExtensionDecl, message: &'static str) -> (ExtensionDecl, HostFn) {
    (decl, Box::new(move |_args| Err(message.to_string())))
}

/// A host implementation that must not be called.
fn never_called(decl: ExtensionDecl) -> (ExtensionDecl, HostFn) {
    (
        decl,
        Box::new(|_args| panic!("the host must not be called")),
    )
}

/// `paramRef.name`: the module calls `kw.k8s.get` with `apiVersion`, `kind`,
/// `namespace`, and `name`. The validation reads the returned ConfigMap.
#[rstest]
#[case::accept(3, Expected::Accepted)]
#[case::reject(10, Expected::rejected("limit 5 from replica-policy", 422))]
fn test_vap_params_by_name(#[case] replicas: i64, #[case] expected: Expected) {
    let bindings = params_bindings(
        serde_json::json!({ "name": "replica-policy", "namespace": "default" }),
        replicas,
    );
    let result = eval_vap(
        PARAMS_SPEC,
        &bindings,
        Some((
            vap::kw_k8s_get_extension(),
            Box::new(|args| {
                let map = &args[0];
                assert_eq!(map["apiVersion"], "v1");
                assert_eq!(map["kind"], "ConfigMap");
                assert_eq!(map["name"], "replica-policy");
                assert_eq!(map["namespace"], "default");
                assert!(
                    map.get("labelSelector").is_none(),
                    "get request must not carry a labelSelector"
                );
                Ok(params_configmap("replica-policy", "5"))
            }),
        )),
    );
    assert_outcome(result, &expected);
}

/// How the module resolves `params` for each combination of `paramRef`
/// shape, host response, and `parameterNotFoundAction`.
///
/// - A host error under `Deny` (the default) traps before any validation
///   runs. The error keeps its `kw.k8s.get` or `kw.k8s.list` origin, so a
///   host can tell a failed `params` lookup apart from other runtime errors.
/// - An empty `list` result under `Deny` traps with `no parameters found`.
/// - Under `Allow`, both cases accept the request. A host error counts as
///   "not found". See LIMITATIONS.md.
/// - With several params, the module evaluates the policy once per param and
///   reports the first rejection.
#[rstest]
#[case::name_host_error_deny(
    name_ref(None),
    failing(vap::kw_k8s_get_extension(), "configmap not found"),
    3,
    Expected::ExtensionError {
        message: "configmap not found",
        namespace: Some("kw.k8s"),
        function: "get",
    }
)]
#[case::name_host_error_allow(
    name_ref(Some("Allow")),
    failing(vap::kw_k8s_get_extension(), "configmap not found"),
    100,
    Expected::Accepted
)]
#[case::selector_empty_deny(
    selector_ref(None),
    list_returning(vec![]),
    1,
    Expected::Error("no parameters found")
)]
#[case::selector_empty_allow(
    selector_ref(Some("Allow")),
    list_returning(vec![]),
    100,
    Expected::Accepted
)]
#[case::selector_host_error_deny(
    selector_ref(None),
    failing(vap::kw_k8s_list_extension(), "forbidden"),
    1,
    Expected::ExtensionError {
        message: "forbidden",
        namespace: Some("kw.k8s"),
        function: "list",
    }
)]
#[case::selector_host_error_allow(
    selector_ref(Some("Allow")),
    failing(vap::kw_k8s_list_extension(), "forbidden"),
    100,
    Expected::Accepted
)]
#[case::selector_all_pass(
    selector_ref(None),
    list_returning(vec![params_configmap("first", "5"), params_configmap("second", "10")]),
    3,
    Expected::Accepted
)]
#[case::selector_first_denial_wins(
    selector_ref(None),
    list_returning(vec![params_configmap("first", "5"), params_configmap("second", "10")]),
    100,
    Expected::rejected("limit 5 from first", 422)
)]
fn test_vap_params_resolution(
    #[case] param_ref: serde_json::Value,
    #[case] host: (ExtensionDecl, HostFn),
    #[case] replicas: i64,
    #[case] expected: Expected,
) {
    let bindings = params_bindings(param_ref, replicas);
    assert_outcome(eval_vap(PARAMS_SPEC, &bindings, Some(host)), &expected);
}

/// `selector` ref, 2 items, only the second violates → rejected with the
/// second's message. The host receives the formatted, sorted label selector
/// and the `paramRef.namespace`.
#[test]
fn test_vap_params_selector_second_param_rejects() {
    let bindings = params_bindings(
        serde_json::json!({
            "namespace": "config",
            "selector": {
                "matchLabels": { "env": "test" },
                "matchExpressions": [
                    { "key": "tier", "operator": "In", "values": ["web", "api"] },
                    { "key": "archived", "operator": "DoesNotExist" }
                ]
            }
        }),
        7,
    );
    let result = eval_vap(
        PARAMS_SPEC,
        &bindings,
        Some((
            vap::kw_k8s_list_extension(),
            Box::new(|args| {
                let map = &args[0];
                assert_eq!(map["apiVersion"], "v1");
                assert_eq!(map["kind"], "ConfigMap");
                assert_eq!(map["namespace"], "config");
                assert_eq!(
                    map["labelSelector"], "!archived,env=test,tier in (api,web)",
                    "label selector is not formatted and sorted"
                );
                assert!(
                    map.get("name").is_none(),
                    "list request must not carry a name"
                );
                Ok(serde_json::json!({
                    "items": [params_configmap("loose", "10"), params_configmap("strict", "5")]
                }))
            }),
        )),
    );
    assert_outcome(result, &Expected::rejected("limit 5 from strict", 422));
}

/// A policy whose `matchCondition` reads `params`. Kubernetes evaluates
/// `matchConditions` once per param.
const PARAMS_MATCH_CONDITION_SPEC: &str = r#"spec:
  paramKind:
    apiVersion: v1
    kind: ConfigMap
  matchConditions:
    - name: enabled
      expression: "params.data.enabled == 'true'"
  validations:
    - expression: "object.spec.replicas <= int(params.data.maxReplicas)"
      messageExpression: "'limit ' + params.data.maxReplicas + ' from ' + params.metadata.name"
"#;

fn params_configmap_enabled(name: &str, max_replicas: &str, enabled: bool) -> serde_json::Value {
    let mut cm = params_configmap(name, max_replicas);
    cm["data"]["enabled"] = serde_json::Value::String(enabled.to_string());
    cm
}

/// A false `matchCondition` skips that param only. The object has 7
/// replicas. The first param is always disabled. The outcome depends on the
/// second, enabled, param.
#[rstest]
#[case::disabled_param_is_skipped(
    vec![
        params_configmap_enabled("disabled-strict", "1", false),
        params_configmap_enabled("enabled-loose", "10", true),
    ],
    Expected::Accepted
)]
#[case::enabled_param_rejects(
    vec![
        params_configmap_enabled("disabled-loose", "10", false),
        params_configmap_enabled("enabled-strict", "5", true),
    ],
    Expected::rejected("limit 5 from enabled-strict", 422)
)]
fn test_vap_params_match_condition_per_param(
    #[case] items: Vec<serde_json::Value>,
    #[case] expected: Expected,
) {
    let bindings = params_bindings(selector_ref(None), 7);
    let result = eval_vap(
        PARAMS_MATCH_CONDITION_SPEC,
        &bindings,
        Some(list_returning(items)),
    );
    assert_outcome(result, &expected);
}

/// `paramRef.namespace` empty → the host receives `request.namespace`.
#[test]
fn test_vap_params_namespace_defaults_to_request_namespace() {
    let bindings = params_bindings(name_ref(None), 3);
    let result = eval_vap(
        PARAMS_SPEC,
        &bindings,
        Some((
            vap::kw_k8s_get_extension(),
            Box::new(|args| {
                assert_eq!(
                    args[0]["namespace"], "team-a",
                    "namespace must default to request.namespace"
                );
                Ok(params_configmap("replica-policy", "5"))
            }),
        )),
    );
    assert_outcome(result, &Expected::Accepted);
}

/// `paramRef.namespace` empty and no `request` binding → the host receives `""`.
#[test]
fn test_vap_params_namespace_empty_without_request() {
    let bindings = serde_json::json!({
        "paramRef": name_ref(None),
        "object": { "spec": { "replicas": 3 } }
    })
    .to_string();
    let result = eval_vap(
        PARAMS_SPEC,
        &bindings,
        Some((
            vap::kw_k8s_get_extension(),
            Box::new(|args| {
                assert_eq!(args[0]["namespace"], "");
                Ok(params_configmap("replica-policy", "5"))
            }),
        )),
    );
    assert_outcome(result, &Expected::Accepted);
}

/// A `variables` entry can read `params`. Kubernetes binds `params` before
/// `variables`.
#[test]
fn test_vap_params_visible_in_variables() {
    let spec = r#"spec:
  paramKind:
    apiVersion: v1
    kind: ConfigMap
  variables:
    - name: limit
      expression: "int(params.data.maxReplicas)"
  validations:
    - expression: "object.spec.replicas <= variables.limit"
      message: "over the limit"
"#;
    let bindings = params_bindings(name_ref(None), 7);
    let result = eval_vap(
        spec,
        &bindings,
        Some((
            vap::kw_k8s_get_extension(),
            Box::new(|_args| Ok(params_configmap("replica-policy", "5"))),
        )),
    );
    assert_outcome(result, &Expected::rejected("over the limit", 422));
}

/// `paramRef` with neither `name` nor `selector` is a malformed binding. It
/// is an error even under `Allow`. The host is never called.
#[rstest]
#[case::deny(serde_json::json!({ "namespace": "default" }))]
#[case::allow(serde_json::json!({ "namespace": "default", "parameterNotFoundAction": "Allow" }))]
fn test_vap_params_ref_without_name_or_selector_is_error(#[case] param_ref: serde_json::Value) {
    let bindings = params_bindings(param_ref, 3);
    let result = eval_vap(
        PARAMS_SPEC,
        &bindings,
        Some(never_called(vap::kw_k8s_get_extension())),
    );
    assert_outcome(
        result,
        &Expected::Error("paramRef must have either name or selector"),
    );
}

/// `paramKind` set but no `paramRef` in the bindings → error. The host is
/// never called.
#[test]
fn test_vap_params_ref_missing_is_error() {
    let bindings = serde_json::json!({ "object": { "spec": { "replicas": 3 } } }).to_string();
    let result = eval_vap(
        PARAMS_SPEC,
        &bindings,
        Some(never_called(vap::kw_k8s_get_extension())),
    );
    assert_outcome(result, &Expected::Error("paramRef binding is missing"));
}

// ─── kw.k8s builder chain coverage ───────────────────────────────────────────
//
// These tests exercise the builder chain compiler via CEL `variables` expressions
// (Option C): each variable calls kw.k8s.apiVersion(...).kind(...)[.chain()...].terminal()
// directly in CEL, and the validation expression references the result.
// The host callback receives the accumulated builder map as args[0] and can
// assert on the fields that were set.

/// list() terminal — host returns 2 items → validation passes (size >= 1).
#[test]
fn test_vap_kw_k8s_list_accept() {
    let spec = r#"spec:
  variables:
    - name: deploys
      expression: "kw.k8s.apiVersion('apps/v1').kind('Deployment').list()"
  validations:
    - expression: "variables.deploys.items.size() >= 1"
      message: "no deployments found"
"#;
    let bindings = serde_json::json!({ "object": { "kind": "Namespace" } }).to_string();

    let result = eval_vap(
        spec,
        &bindings,
        Some((
            vap::kw_k8s_list_extension(),
            Box::new(|args| {
                let map = &args[0];
                assert_eq!(
                    map["apiVersion"], "apps/v1",
                    "wrong apiVersion in builder map"
                );
                assert_eq!(map["kind"], "Deployment", "wrong kind in builder map");
                Ok(serde_json::json!({
                    "items": [
                        { "metadata": { "name": "deploy-a" } },
                        { "metadata": { "name": "deploy-b" } }
                    ]
                }))
            }),
        )),
    );
    assert_outcome(result, &Expected::Accepted);
}

/// list() terminal — host returns empty list → validation fails.
#[test]
fn test_vap_kw_k8s_list_reject() {
    let spec = r#"spec:
  variables:
    - name: deploys
      expression: "kw.k8s.apiVersion('apps/v1').kind('Deployment').list()"
  validations:
    - expression: "variables.deploys.items.size() >= 1"
      message: "no deployments found"
"#;
    let bindings = serde_json::json!({ "object": { "kind": "Namespace" } }).to_string();

    let result = eval_vap(
        spec,
        &bindings,
        Some((
            vap::kw_k8s_list_extension(),
            Box::new(|_args| Ok(serde_json::json!({ "items": [] }))),
        )),
    );
    assert_outcome(result, &Expected::rejected("no deployments found", 422));
}

/// .namespace() chain step is forwarded to the host inside the builder map.
#[test]
fn test_vap_kw_k8s_list_with_namespace() {
    let spec = r#"spec:
  variables:
    - name: deploys
      expression: "kw.k8s.apiVersion('apps/v1').kind('Deployment').namespace('prod').list()"
  validations:
    - expression: "variables.deploys.items.size() >= 1"
      message: "no prod deployments"
"#;
    let bindings = serde_json::json!({ "object": { "kind": "Namespace" } }).to_string();

    let result = eval_vap(
        spec,
        &bindings,
        Some((
            vap::kw_k8s_list_extension(),
            Box::new(|args| {
                let map = &args[0];
                assert_eq!(map["apiVersion"], "apps/v1");
                assert_eq!(map["kind"], "Deployment");
                assert_eq!(map["namespace"], "prod", "namespace not forwarded to host");
                Ok(serde_json::json!({
                    "items": [{ "metadata": { "name": "prod-deploy", "namespace": "prod" } }]
                }))
            }),
        )),
    );
    assert_outcome(result, &Expected::Accepted);
}

/// .labelSelector() chain step is forwarded to the host inside the builder map.
#[test]
fn test_vap_kw_k8s_list_with_label_selector() {
    let spec = r#"spec:
  variables:
    - name: webDeploys
      expression: "kw.k8s.apiVersion('apps/v1').kind('Deployment').labelSelector('app=web').list()"
  validations:
    - expression: "variables.webDeploys.items.size() == 1"
      message: "expected exactly one web deployment"
"#;
    let bindings = serde_json::json!({ "object": { "kind": "Namespace" } }).to_string();

    let result = eval_vap(
        spec,
        &bindings,
        Some((
            vap::kw_k8s_list_extension(),
            Box::new(|args| {
                let map = &args[0];
                assert_eq!(
                    map["labelSelector"], "app=web",
                    "labelSelector not forwarded"
                );
                Ok(serde_json::json!({
                    "items": [{ "metadata": { "name": "web-deploy" } }]
                }))
            }),
        )),
    );
    assert_outcome(result, &Expected::Accepted);
}

// ─── kw.k8s chain step coverage ──────────────────────────────────────────────

/// `.fieldSelector()` chain step is forwarded to the host inside the builder map.
///
/// Cases:
/// - `matching_result_accepted`  — host returns one Pod  → `size() >= 1` → accepted.
/// - `empty_result_rejected`     — host returns no Pods  → `size() >= 1` → rejected.
#[rstest]
#[case::matching_result_accepted(
    serde_json::json!({ "items": [{ "metadata": { "name": "pod-a" } }] }),
    Expected::Accepted
)]
#[case::empty_result_rejected(
    serde_json::json!({ "items": [] }),
    Expected::rejected("no running pods found", 422)
)]
fn test_vap_kw_k8s_field_selector(
    #[case] host_response: serde_json::Value,
    #[case] expected: Expected,
) {
    let spec = r#"spec:
  variables:
    - name: pods
      expression: "kw.k8s.apiVersion('v1').kind('Pod').fieldSelector('status.phase=Running').list()"
  validations:
    - expression: "variables.pods.items.size() >= 1"
      message: "no running pods found"
"#;
    let bindings = serde_json::json!({ "object": { "kind": "Namespace" } }).to_string();

    let result = eval_vap(
        spec,
        &bindings,
        Some((
            vap::kw_k8s_list_extension(),
            Box::new(move |args| {
                let map = &args[0];
                assert_eq!(map["apiVersion"], "v1", "wrong apiVersion");
                assert_eq!(map["kind"], "Pod", "wrong kind");
                assert_eq!(
                    map["fieldSelector"], "status.phase=Running",
                    "fieldSelector not forwarded to host"
                );
                Ok(host_response.clone())
            }),
        )),
    );
    assert_outcome(result, &expected);
}

/// `.fieldMask()` chain step with a single mask — host receives `fieldMasks`
/// as a single-element array.
///
/// Exercises the `accumulate = true` code path in `cel_builder_step` for the
/// first call (None → Array([val])).
#[test]
fn test_vap_kw_k8s_field_mask_single() {
    let spec = r#"spec:
  variables:
    - name: cms
      expression: "kw.k8s.apiVersion('v1').kind('ConfigMap').fieldMask('metadata.name').list()"
  validations:
    - expression: "variables.cms.items.size() >= 1"
      message: "no ConfigMaps found"
"#;
    let bindings = serde_json::json!({ "object": { "kind": "Namespace" } }).to_string();

    let result = eval_vap(
        spec,
        &bindings,
        Some((
            vap::kw_k8s_list_extension(),
            Box::new(|args| {
                let map = &args[0];
                assert_eq!(map["apiVersion"], "v1");
                assert_eq!(map["kind"], "ConfigMap");
                assert_eq!(
                    map["fieldMasks"],
                    serde_json::json!(["metadata.name"]),
                    "expected single-element fieldMasks array"
                );
                Ok(serde_json::json!({
                    "items": [{ "metadata": { "name": "cm-a" } }]
                }))
            }),
        )),
    );
    assert_outcome(result, &Expected::Accepted);
}

/// `.fieldMask()` chain step called twice — host receives `fieldMasks` as a
/// two-element array.
///
/// Exercises the `accumulate = true` array-append path in `cel_builder_step`:
/// the second call turns `Array([a])` into `Array([a, b])`.
#[test]
fn test_vap_kw_k8s_field_mask_accumulated() {
    let spec = r#"spec:
  variables:
    - name: cms
      expression: "kw.k8s.apiVersion('v1').kind('ConfigMap').fieldMask('metadata.name').fieldMask('data').list()"
  validations:
    - expression: "variables.cms.items.size() >= 1"
      message: "no ConfigMaps found"
"#;
    let bindings = serde_json::json!({ "object": { "kind": "Namespace" } }).to_string();

    let result = eval_vap(
        spec,
        &bindings,
        Some((
            vap::kw_k8s_list_extension(),
            Box::new(|args| {
                let map = &args[0];
                assert_eq!(map["apiVersion"], "v1");
                assert_eq!(map["kind"], "ConfigMap");
                assert_eq!(
                    map["fieldMasks"],
                    serde_json::json!(["metadata.name", "data"]),
                    "fieldMask calls must accumulate into an array in call order"
                );
                Ok(serde_json::json!({
                    "items": [{ "metadata": { "name": "cm-a" } }]
                }))
            }),
        )),
    );
    assert_outcome(result, &Expected::Accepted);
}

/// `.namespace()` + `.get()` — both namespace and name reach the host; validation
/// reads a field from the returned resource.
#[test]
fn test_vap_kw_k8s_get_with_namespace() {
    let spec = r#"spec:
  variables:
    - name: cfg
      expression: "kw.k8s.apiVersion('v1').kind('ConfigMap').namespace('default').get('my-config')"
  validations:
    - expression: "variables.cfg.data.key == 'expected-value'"
      message: "config key mismatch"
"#;
    let bindings = serde_json::json!({ "object": { "kind": "Deployment" } }).to_string();

    let result = eval_vap(
        spec,
        &bindings,
        Some((
            vap::kw_k8s_get_extension(),
            Box::new(|args| {
                let map = &args[0];
                assert_eq!(map["apiVersion"], "v1");
                assert_eq!(map["kind"], "ConfigMap");
                assert_eq!(map["namespace"], "default", "namespace not forwarded");
                assert_eq!(map["name"], "my-config", "name not forwarded");
                Ok(serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "ConfigMap",
                    "metadata": { "name": "my-config", "namespace": "default" },
                    "data": { "key": "expected-value" }
                }))
            }),
        )),
    );
    assert_outcome(result, &Expected::Accepted);
}

// ─── namespaceObject ───────────────────────────────────────────────────────────
//
// These tests cover the `namespaceObject` resolution done by the runtime:
// the Namespace-kind special case, cluster-scoped requests, the `request`
// binding contract, and host error propagation.

/// The policy used by most namespaceObject tests. Fails unless
/// `namespaceObject` is non-null and its `metadata.name` is `team-a`.
const NAMESPACE_OBJECT_SPEC: &str = r#"spec:
  validations:
    - expression: "namespaceObject != null && namespaceObject.metadata.name == 'team-a'"
      message: "missing or wrong namespaceObject"
"#;

/// A policy that asserts `namespaceObject` is `null`.
const NAMESPACE_OBJECT_NULL_SPEC: &str = r#"spec:
  validations:
    - expression: "namespaceObject == null"
      message: "expected a null namespaceObject"
"#;

fn namespace_object_json(name: &str) -> serde_json::Value {
    serde_json::json!({
        "apiVersion": "v1",
        "kind": "Namespace",
        "metadata": { "name": name }
    })
}

/// A `kw.k8s.get` host implementation for the Namespace lookup. Asserts the
/// request map has no `namespace` key (the Namespace resource is
/// cluster-scoped), then returns `response`.
fn namespace_get(response: serde_json::Value) -> (ExtensionDecl, HostFn) {
    (
        vap::kw_k8s_get_extension(),
        Box::new(move |args| {
            let map = &args[0];
            assert_eq!(map["apiVersion"], "v1");
            assert_eq!(map["kind"], "Namespace");
            assert!(
                map.get("namespace").is_none(),
                "the Namespace fetch must not carry a `namespace` key"
            );
            Ok(response.clone())
        }),
    )
}

/// A namespaced request: one `kw.k8s.get` call for `request.namespace`, with
/// `name` forwarded, and the result visible to the validation.
#[test]
fn test_vap_namespace_object_namespaced_request() {
    let bindings = serde_json::json!({
        "request": { "namespace": "team-a" },
        "object": { "kind": "Deployment", "metadata": { "namespace": "team-a" } }
    })
    .to_string();
    let result = eval_vap(
        NAMESPACE_OBJECT_SPEC,
        &bindings,
        Some((
            vap::kw_k8s_get_extension(),
            Box::new(|args| {
                assert_eq!(args[0]["name"], "team-a");
                Ok(namespace_object_json("team-a"))
            }),
        )),
    );
    assert_outcome(result, &Expected::Accepted);
}

/// A cluster-scoped request (`request.namespace` missing, then empty):
/// `namespaceObject` is `null`, and the host is never called.
#[rstest]
#[case::namespace_missing(serde_json::json!({}))]
#[case::namespace_empty(serde_json::json!({ "namespace": "" }))]
fn test_vap_namespace_object_cluster_scoped_request(#[case] request: serde_json::Value) {
    let bindings = serde_json::json!({ "request": request, "object": {} }).to_string();
    let result = eval_vap(
        NAMESPACE_OBJECT_NULL_SPEC,
        &bindings,
        Some(never_called(vap::kw_k8s_get_extension())),
    );
    assert_outcome(result, &Expected::Accepted);
}

/// The resource under admission is itself a `v1/Namespace`. `request.name`
/// and `request.namespace` are the same value (the Kubernetes special
/// case). `namespaceObject` is `null`, and the host is never called — this
/// matters most on `CREATE`, where the Namespace does not exist yet.
#[test]
fn test_vap_namespace_object_namespace_kind_request() {
    let bindings = serde_json::json!({
        "request": {
            "namespace": "team-a",
            "name": "team-a",
            "kind": { "group": "", "version": "v1", "kind": "Namespace" }
        },
        "object": { "kind": "Namespace", "metadata": { "name": "team-a" } }
    })
    .to_string();
    let result = eval_vap(
        NAMESPACE_OBJECT_NULL_SPEC,
        &bindings,
        Some(never_called(vap::kw_k8s_get_extension())),
    );
    assert_outcome(result, &Expected::Accepted);
}

/// The policy references `namespaceObject`, but the `request` binding is
/// absent entirely. The host contract requires `request` in this case; the
/// host is never called.
#[test]
fn test_vap_namespace_object_missing_request_is_error() {
    let bindings = serde_json::json!({ "object": {} }).to_string();
    let result = eval_vap(
        NAMESPACE_OBJECT_SPEC,
        &bindings,
        Some(never_called(vap::kw_k8s_get_extension())),
    );
    assert_outcome(result, &Expected::Error("request binding is missing"));
}

/// A host error fetching the Namespace surfaces as a runtime error with
/// origin `kw.k8s.get`, like a failed `params` fetch.
#[test]
fn test_vap_namespace_object_host_error() {
    let bindings = serde_json::json!({
        "request": { "namespace": "team-a" },
        "object": {}
    })
    .to_string();
    let result = eval_vap(
        NAMESPACE_OBJECT_SPEC,
        &bindings,
        Some(failing(
            vap::kw_k8s_get_extension(),
            "namespaces \"team-a\" not found",
        )),
    );
    assert_outcome(
        result,
        &Expected::ExtensionError {
            message: "namespaces \"team-a\" not found",
            namespace: Some("kw.k8s"),
            function: "get",
        },
    );
}

/// A policy that never reads `namespaceObject` makes no `kw.k8s.get` call,
/// even with a namespaced request. No extension needs to be registered.
#[test]
fn test_vap_namespace_object_not_referenced_makes_no_call() {
    let spec = r#"spec:
  validations:
    - expression: "object.spec.replicas <= 5"
      message: "too many replicas"
"#;
    let bindings = serde_json::json!({
        "request": { "namespace": "team-a" },
        "object": { "spec": { "replicas": 3 } }
    })
    .to_string();
    assert_outcome(eval_vap(spec, &bindings, None), &Expected::Accepted);
}

/// A host that still passes `namespaceObject` directly in the bindings does
/// not win: the guest's own resolution overwrites it.
#[test]
fn test_vap_namespace_object_host_binding_is_overwritten() {
    let bindings = serde_json::json!({
        "request": { "namespace": "team-a" },
        "object": {},
        // Stale value a legacy (0.10-style) host might still pass. The
        // guest must overwrite it with the host's `kw.k8s.get` response.
        "namespaceObject": namespace_object_json("stale-value")
    })
    .to_string();
    let result = eval_vap(
        NAMESPACE_OBJECT_SPEC,
        &bindings,
        Some(namespace_get(namespace_object_json("team-a"))),
    );
    assert_outcome(result, &Expected::Accepted);
}

/// A `matchCondition` that reads `namespaceObject` sees the fetched value —
/// the fetch runs before matchConditions are evaluated.
#[test]
fn test_vap_namespace_object_visible_in_match_condition() {
    let spec = r#"spec:
  matchConditions:
    - name: team-a-only
      expression: "namespaceObject.metadata.name == 'team-a'"
  validations:
    - expression: "false"
      message: "should never run for non-team-a namespaces"
"#;
    let bindings = serde_json::json!({
        "request": { "namespace": "team-b" },
        "object": {}
    })
    .to_string();
    let result = eval_vap(
        spec,
        &bindings,
        Some(namespace_get(namespace_object_json("team-b"))),
    );
    // matchCondition is false (team-b != team-a) → this param (the only one,
    // since there is no paramKind) is skipped → accepted, and the always
    // failing validation never runs.
    assert_outcome(result, &Expected::Accepted);
}

/// A `DELETE`-style request (`object` is `null`) still resolves
/// `namespaceObject` from `request.namespace`.
#[test]
fn test_vap_namespace_object_delete_request() {
    let bindings = serde_json::json!({
        "request": { "namespace": "team-a" },
        "object": null
    })
    .to_string();
    let result = eval_vap(
        NAMESPACE_OBJECT_SPEC,
        &bindings,
        Some(namespace_get(namespace_object_json("team-a"))),
    );
    assert_outcome(result, &Expected::Accepted);
}

/// `namespaceObject` together with `paramRef.selector`: both `kw.k8s.get`
/// (for the Namespace) and `kw.k8s.list` (for `params`) are served in the
/// same evaluation.
#[test]
fn test_vap_namespace_object_with_params_selector() {
    let spec = r#"spec:
  paramKind:
    apiVersion: v1
    kind: ConfigMap
  validations:
    - expression: "namespaceObject.metadata.name == 'team-a' &&
        object.spec.replicas <= int(params.data.maxReplicas)"
      message: "rejected"
"#;
    let bindings = params_bindings(selector_ref(None), 3);
    let result = eval_vap_with_extensions(
        spec,
        &bindings,
        vec![
            namespace_get(namespace_object_json("team-a")),
            list_returning(vec![params_configmap("cfg", "5")]),
        ],
    );
    assert_outcome(result, &Expected::Accepted);
}
