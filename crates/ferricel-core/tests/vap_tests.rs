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
/// (e.g. for `kw.k8s...`) on the `Engine`.
fn eval_vap(
    spec_body: &str,
    bindings_json: &str,
    extension: Option<(ExtensionDecl, HostFn)>,
) -> Result<serde_json::Value, anyhow::Error> {
    let logger = test_logger();
    let wasm_bytes = Builder::new()
        .with_logger(logger.clone())
        .build()
        .compile_vap(&vap_yaml(spec_body))?;

    let mut runtime_builder = runtime::Builder::new()
        .with_logger(logger)
        .with_log_level(LogLevel::Info)
        .with_wasm(wasm_bytes);
    if let Some((decl, implementation)) = extension {
        runtime_builder = runtime_builder.with_extension(decl, implementation);
    }
    let result_str = runtime_builder.build()?.eval(Some(bindings_json))?;

    Ok(serde_json::from_str(&result_str)?)
}

// ─── Expected outcome + single assertion ───────────────────────────────────

/// The three possible outcomes of evaluating a compiled VAP module.
#[derive(Debug, Clone)]
enum Expected {
    /// `{"accepted": true}`, with no `message` field.
    Accepted,
    /// `{"accepted": false, ...}`, optionally asserting `message` and/or `code`.
    Rejected {
        message: Option<&'static str>,
        code: Option<i32>,
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
}

/// Assert that `result` matches `expected`.
fn assert_outcome(result: Result<serde_json::Value, anyhow::Error>, expected: &Expected) {
    match expected {
        Expected::Accepted => {
            let result = result.expect("expected an accepted response, got an error");
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
        Expected::Rejected { message, code } => {
            let result = result.expect("expected a rejected response, got an error");
            assert_eq!(
                result.get("accepted"),
                Some(&serde_json::Value::Bool(false)),
                "expected accepted=false, got: {result}"
            );
            if let Some(expected_msg) = message {
                assert_eq!(
                    result.get("message").and_then(|v| v.as_str()),
                    Some(*expected_msg),
                    "unexpected rejection message, got: {result}"
                );
            }
            if let Some(expected_code) = code {
                assert_eq!(
                    result.get("code").and_then(|v| v.as_i64()),
                    Some(i64::from(*expected_code)),
                    "unexpected rejection code, got: {result}"
                );
            }
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
        &Expected::Rejected {
            message: Some("too many replicas"),
            code: None,
        },
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
        &Expected::Rejected {
            message: Some("replicas 7 exceeds limit 5"),
            code: None,
        },
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
        &Expected::Rejected {
            message: Some("too many replicas"),
            code: None,
        },
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

// ─── Runtime errors ───────────────────────────────────────────────────────────
//
// A CEL runtime error in a matchCondition or validation must surface to the
// host as an `Err` from `eval()` (the module traps), never as a silent accept
// or a rejection. This lets the host apply the policy's `failurePolicy`.

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

/// An erroring validation is not masked by earlier passing validations.
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

/// A matchCondition whose expression errors → `eval()` returns `Err`, rather
/// than treating the condition as `true` and running the validations.
#[test]
fn test_vap_match_condition_runtime_error_is_surfaced() {
    let spec = r#"spec:
  matchConditions:
    - name: broken
      expression: "(1 / 0) == 1"
  validations:
    - expression: "true"
"#;
    assert_outcome(
        eval_vap(spec, EMPTY_OBJECT_BINDINGS, None),
        &Expected::Error("divide by zero"),
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
        Some((
            vap::kw_k8s_get_extension(),
            Box::new(|_args| Err("boom".to_string())),
        )),
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
