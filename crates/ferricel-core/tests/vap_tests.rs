//! Integration tests for VAP (ValidatingAdmissionPolicy) compilation.
//!
//! Each test compiles a VAP `spec:` YAML fragment and executes it with JSON
//! bindings, then asserts the resulting `ValidationResponse`-style JSON (or
//! runtime error) via [`Expected`] / [`assert_outcome`].

use ferricel_core::{
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
    /// The module traps: `eval()` returns `Err` whose message contains
    /// `"CEL runtime error"` and the given needle.
    Error(&'static str),
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
        Expected::Error(needle) => {
            let err = result.expect_err("expected a runtime error, got a response");
            let msg = format!("{err:#}");
            assert!(
                msg.contains("CEL runtime error"),
                "expected a CEL runtime error, got: {msg}"
            );
            assert!(
                msg.contains(needle),
                "expected {needle:?} in error, got: {msg}"
            );
        }
    }
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
#[test]
fn test_vap_variables() {
    let spec = r#"spec:
  variables:
    - name: maxReplicas
      expression: "5"
  validations:
    - expression: "object.spec.replicas <= variables.maxReplicas"
      message: "too many replicas"
"#;

    let bindings_ok = serde_json::json!({ "object": { "spec": { "replicas": 4 } } }).to_string();
    assert_outcome(eval_vap(spec, &bindings_ok, None), &Expected::Accepted);

    let bindings_fail = serde_json::json!({ "object": { "spec": { "replicas": 10 } } }).to_string();
    assert_outcome(
        eval_vap(spec, &bindings_fail, None),
        &Expected::rejected_any(),
    );
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
/// silently accepted.
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
    assert_outcome(result, &Expected::Error("boom"));
}

/// A failed `params` fetch propagates into the validation that uses `params`.
#[test]
fn test_vap_params_fetch_error_is_surfaced() {
    let spec = r#"spec:
  paramKind:
    apiVersion: v1
    kind: ConfigMap
  validations:
    - expression: "object.spec.replicas <= int(params.data.maxReplicas)"
"#;
    let bindings = serde_json::json!({
        "paramRef": { "name": "replica-policy", "namespace": "default" },
        "object": { "spec": { "replicas": 3 } }
    })
    .to_string();

    let result = eval_vap(
        spec,
        &bindings,
        Some((
            vap::kw_k8s_get_extension(),
            Box::new(|_args| Err("configmap not found".to_string())),
        )),
    );
    assert_outcome(result, &Expected::Error("configmap not found"));
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

// ─── kw.k8s params tests ──────────────────────────────────────────────────────

/// A policy that uses `paramKind` to fetch a ConfigMap holding policy config,
/// then validates that the incoming Deployment's replica count does not exceed
/// the limit stored in `params.data.maxReplicas`.
/// Host returns maxReplicas="5"; object has replicas=3 → accepted.
#[test]
fn test_vap_params_kw_k8s_accept() {
    let spec = r#"spec:
  paramKind:
    apiVersion: v1
    kind: ConfigMap
  validations:
    - expression: "object.spec.replicas <= int(params.data.maxReplicas)"
      message: "replicas exceeds the configured maximum"
"#;
    let bindings = serde_json::json!({
        "paramRef": { "name": "replica-policy", "namespace": "default" },
        "object": {
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": { "name": "my-app" },
            "spec": { "replicas": 3 }
        }
    })
    .to_string();

    let result = eval_vap(
        spec,
        &bindings,
        Some((
            vap::kw_k8s_get_extension(),
            Box::new(|args| {
                let map = &args[0];
                assert_eq!(map["apiVersion"], "v1");
                assert_eq!(map["kind"], "ConfigMap");
                assert_eq!(map["name"], "replica-policy");
                assert_eq!(map["namespace"], "default");
                Ok(serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "ConfigMap",
                    "metadata": { "name": "replica-policy", "namespace": "default" },
                    "data": { "maxReplicas": "5" }
                }))
            }),
        )),
    );
    assert_outcome(result, &Expected::Accepted);
}

/// Same policy; object has replicas=10 which exceeds maxReplicas="5" → rejected.
#[test]
fn test_vap_params_kw_k8s_reject() {
    let spec = r#"spec:
  paramKind:
    apiVersion: v1
    kind: ConfigMap
  validations:
    - expression: "object.spec.replicas <= int(params.data.maxReplicas)"
      message: "replicas exceeds the configured maximum"
"#;
    let bindings = serde_json::json!({
        "paramRef": { "name": "replica-policy", "namespace": "default" },
        "object": {
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": { "name": "my-app" },
            "spec": { "replicas": 10 }
        }
    })
    .to_string();

    let result = eval_vap(
        spec,
        &bindings,
        Some((
            vap::kw_k8s_get_extension(),
            Box::new(|_args| {
                Ok(serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "ConfigMap",
                    "metadata": { "name": "replica-policy", "namespace": "default" },
                    "data": { "maxReplicas": "5" }
                }))
            }),
        )),
    );
    assert_outcome(
        result,
        &Expected::rejected("replicas exceeds the configured maximum", 422),
    );
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
