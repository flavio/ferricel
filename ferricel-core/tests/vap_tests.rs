//! Integration tests for VAP (ValidatingAdmissionPolicy) compilation.
//!
//! Each test compiles a VapSpec (or YAML) and executes it with JSON bindings,
//! then asserts the resulting `ValidationResponse`-style JSON.

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

/// Compile a VAP YAML string to Wasm, then evaluate it with the given JSON
/// bindings, returning the parsed `serde_json::Value`.
fn compile_vap_and_eval(
    vap_yaml: &str,
    bindings_json: &str,
) -> Result<serde_json::Value, anyhow::Error> {
    let logger = test_logger();
    let wasm_bytes = Builder::new()
        .with_logger(logger.clone())
        .build()
        .compile_vap(vap_yaml)?;

    let result_str = runtime::Builder::new()
        .with_logger(logger)
        .with_log_level(LogLevel::Info)
        .with_wasm(wasm_bytes)
        .build()?
        .eval(Some(bindings_json))?;

    Ok(serde_json::from_str(&result_str)?)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn assert_accepted(result: &serde_json::Value) {
    assert_eq!(
        result.get("accepted"),
        Some(&serde_json::Value::Bool(true)),
        "expected accepted=true, got: {result}"
    );
}

fn assert_rejected(result: &serde_json::Value, message: Option<&str>, code: Option<i32>) {
    assert_eq!(
        result.get("accepted"),
        Some(&serde_json::Value::Bool(false)),
        "expected accepted=false, got: {result}"
    );
    if let Some(expected_msg) = message {
        let actual_msg = result.get("message").and_then(|v| v.as_str()).unwrap_or("");
        assert_eq!(
            actual_msg, expected_msg,
            "unexpected rejection message, got: {result}"
        );
    }
    if let Some(expected_code) = code {
        let actual_code = result.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
        assert_eq!(
            actual_code, expected_code as i64,
            "unexpected rejection code, got: {result}"
        );
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

/// A policy with a single validation that passes → accepted.
#[test]
fn test_vap_accept_simple() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-accept
spec:
  validations:
    - expression: "object.spec.replicas <= 5"
      message: "too many replicas"
"#;

    let bindings = serde_json::json!({
        "object": {
            "spec": {
                "replicas": 3
            }
        }
    })
    .to_string();

    let result = compile_vap_and_eval(yaml, &bindings).unwrap();
    assert_accepted(&result);
}

/// A policy with a single validation that fails → rejected with static message.
#[test]
fn test_vap_reject_static_message() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-reject
spec:
  validations:
    - expression: "object.spec.replicas <= 5"
      message: "too many replicas"
"#;

    let bindings = serde_json::json!({
        "object": {
            "spec": {
                "replicas": 10
            }
        }
    })
    .to_string();

    let result = compile_vap_and_eval(yaml, &bindings).unwrap();
    assert_rejected(&result, Some("too many replicas"), None);
}

/// Validation fails and a `messageExpression` is evaluated to build the message.
#[test]
fn test_vap_reject_message_expression() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-msg-expr
spec:
  validations:
    - expression: "object.spec.replicas <= 5"
      messageExpression: "'replicas ' + string(object.spec.replicas) + ' exceeds limit 5'"
"#;

    let bindings = serde_json::json!({
        "object": {
            "spec": {
                "replicas": 7
            }
        }
    })
    .to_string();

    let result = compile_vap_and_eval(yaml, &bindings).unwrap();
    assert_rejected(&result, Some("replicas 7 exceeds limit 5"), None);
}

/// A matchCondition that evaluates to false → policy skipped → accepted.
#[test]
fn test_vap_match_condition_false_skips_policy() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-match-cond
spec:
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
        "object": {
            "kind": "Pod",
            "spec": {
                "replicas": 99
            }
        }
    })
    .to_string();

    let result = compile_vap_and_eval(yaml, &bindings).unwrap();
    assert_accepted(&result);
}

/// A matchCondition that evaluates to true → validation is enforced → rejected.
#[test]
fn test_vap_match_condition_true_enforces_validation() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-match-cond-true
spec:
  matchConditions:
    - name: only-deployments
      expression: "object.kind == 'Deployment'"
  validations:
    - expression: "object.spec.replicas <= 5"
      message: "too many replicas"
"#;

    let bindings = serde_json::json!({
        "object": {
            "kind": "Deployment",
            "spec": {
                "replicas": 99
            }
        }
    })
    .to_string();

    let result = compile_vap_and_eval(yaml, &bindings).unwrap();
    assert_rejected(&result, None, None);
}

/// Variables are evaluated and accessible in validation expressions.
#[test]
fn test_vap_variables() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-variables
spec:
  variables:
    - name: maxReplicas
      expression: "5"
  validations:
    - expression: "object.spec.replicas <= variables.maxReplicas"
      message: "too many replicas"
"#;

    let bindings_ok = serde_json::json!({
        "object": { "spec": { "replicas": 4 } }
    })
    .to_string();

    let result_ok = compile_vap_and_eval(yaml, &bindings_ok).unwrap();
    assert_accepted(&result_ok);

    let bindings_fail = serde_json::json!({
        "object": { "spec": { "replicas": 10 } }
    })
    .to_string();

    let result_fail = compile_vap_and_eval(yaml, &bindings_fail).unwrap();
    assert_rejected(&result_fail, None, None);
}

/// Multiple validations: first passes, second fails → rejection with second
/// validation's message.
#[test]
fn test_vap_multiple_validations_second_fails() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-multi-val
spec:
  validations:
    - expression: "object.spec.replicas >= 1"
      message: "must have at least 1 replica"
    - expression: "object.spec.replicas <= 5"
      message: "too many replicas"
"#;

    let bindings = serde_json::json!({
        "object": { "spec": { "replicas": 10 } }
    })
    .to_string();

    let result = compile_vap_and_eval(yaml, &bindings).unwrap();
    assert_rejected(&result, Some("too many replicas"), None);
}

/// Validation with a reason maps to the correct HTTP status code.
#[test]
fn test_vap_reason_to_code() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-reason
spec:
  validations:
    - expression: "false"
      message: "forbidden"
      reason: "Forbidden"
"#;

    let bindings = serde_json::json!({}).to_string();
    let result = compile_vap_and_eval(yaml, &bindings).unwrap();
    assert_rejected(&result, Some("forbidden"), Some(403));
}

/// All validations pass → accepted, no rejection fields present.
#[test]
fn test_vap_all_validations_pass() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-all-pass
spec:
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

    let result = compile_vap_and_eval(yaml, &bindings).unwrap();
    assert_accepted(&result);
    assert!(
        result.get("message").is_none(),
        "accepted response should have no message, got: {result}"
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
    true
)]
#[case::no_subjects_field(
    serde_json::json!({ "object": {} }),
    true
)]
#[case::default_sa_subject(
    serde_json::json!({
        "object": {
            "subjects": [
                { "kind": "ServiceAccount", "name": "default", "namespace": "default" }
            ]
        }
    }),
    false
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
    false
)]
fn test_vap_no_default_sa_rolebinding(#[case] object: serde_json::Value, #[case] accepted: bool) {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: "no-default-sa-rolebinding.vap-library.com"
spec:
  failurePolicy: Fail
  validations:
    - expression: "!has(object.subjects) || object.subjects.all(s, !(s.kind == 'ServiceAccount' && s.name == 'default'))"
      message: "subjects cannot include the 'default' service account"
      reason: Invalid
"#;
    let result = compile_vap_and_eval(yaml, &object.to_string()).unwrap();
    if accepted {
        assert_accepted(&result);
    } else {
        assert_rejected(
            &result,
            Some("subjects cannot include the 'default' service account"),
            Some(422),
        );
    }
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
    true
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
    true
)]
#[case::non_pod_kind_skips_all_validations(
    serde_json::json!({ "object": { "kind": "ConfigMap" } }),
    true
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
    false
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
    false
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
    false
)]
fn test_vap_pss_privilege_escalation(#[case] object: serde_json::Value, #[case] accepted: bool) {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: "pss-privilege-escalation.vap-library.com"
spec:
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
    let result = compile_vap_and_eval(yaml, &object.to_string()).unwrap();
    if accepted {
        assert_accepted(&result);
    } else {
        assert_rejected(
            &result,
            Some(
                "securityContext.allowPrivilegeEscalation must be set to false on any containers, initContainers, and ephemeralContainers in Pods",
            ),
            Some(422),
        );
    }
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
    true
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
    true
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
    false
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
    false
)]
#[case::no_security_context(
    serde_json::json!({
        "object": {
            "kind": "Pod",
            "spec": { "containers": [{ "name": "app" }] }
        }
    }),
    false
)]
fn test_vap_pss_capabilities(#[case] object: serde_json::Value, #[case] accepted: bool) {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: "pss-capabilities.vap-library.com"
spec:
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
    let result = compile_vap_and_eval(yaml, &object.to_string()).unwrap();
    if accepted {
        assert_accepted(&result);
    } else {
        assert_rejected(
            &result,
            Some(
                "securityContext.capabilities.drop must include ALL and securityContext.capabilities.add can only include NET_BIND_SERVICE on containers in Pods",
            ),
            Some(422),
        );
    }
}

// ─── kw.k8s params tests ──────────────────────────────────────────────────────

/// A policy that uses `paramKind` to fetch a ConfigMap holding policy config,
/// then validates that the incoming Deployment's replica count does not exceed
/// the limit stored in `params.data.maxReplicas`.
/// Host returns maxReplicas="5"; object has replicas=3 → accepted.
#[test]
fn test_vap_params_kw_k8s_accept() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-params-accept
spec:
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
    });

    let logger = test_logger();
    let wasm_bytes = Builder::new()
        .with_logger(logger.clone())
        .build()
        .compile_vap(yaml)
        .unwrap();

    let result_str = runtime::Builder::new()
        .with_logger(logger)
        .with_log_level(LogLevel::Info)
        .with_wasm(wasm_bytes)
        .with_extension(vap::kw_k8s_get_extension(), |args| {
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
        })
        .build()
        .unwrap()
        .eval(Some(&bindings.to_string()))
        .unwrap();

    let result: serde_json::Value = serde_json::from_str(&result_str).unwrap();
    assert_accepted(&result);
}

/// Same policy; object has replicas=10 which exceeds maxReplicas="5" → rejected.
#[test]
fn test_vap_params_kw_k8s_reject() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-params-reject
spec:
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
    });

    let logger = test_logger();
    let wasm_bytes = Builder::new()
        .with_logger(logger.clone())
        .build()
        .compile_vap(yaml)
        .unwrap();

    let result_str = runtime::Builder::new()
        .with_logger(logger)
        .with_log_level(LogLevel::Info)
        .with_wasm(wasm_bytes)
        .with_extension(vap::kw_k8s_get_extension(), |_args| {
            Ok(serde_json::json!({
                "apiVersion": "v1",
                "kind": "ConfigMap",
                "metadata": { "name": "replica-policy", "namespace": "default" },
                "data": { "maxReplicas": "5" }
            }))
        })
        .build()
        .unwrap()
        .eval(Some(&bindings.to_string()))
        .unwrap();

    let result: serde_json::Value = serde_json::from_str(&result_str).unwrap();
    assert_rejected(
        &result,
        Some("replicas exceeds the configured maximum"),
        Some(422),
    );
}

// ─── kw.k8s builder chain coverage ───────────────────────────────────────────
//
// These tests exercise the builder chain compiler via CEL `variables` expressions
// (Option C): each variable calls kw.k8s.apiVersion(...).kind(...)[.chain()...].terminal()
// directly in CEL, and the validation expression references the result.
// The host callback receives the accumulated builder map as args[0] and can
// assert on the fields that were set.

fn make_kw_k8s_list_decl() -> ExtensionDecl {
    vap::kw_k8s_list_extension()
}

fn make_kw_k8s_get_decl() -> ExtensionDecl {
    vap::kw_k8s_get_extension()
}

/// list() terminal — host returns 2 items → validation passes (size >= 1).
#[test]
fn test_vap_kw_k8s_list_accept() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-list-accept
spec:
  variables:
    - name: deploys
      expression: "kw.k8s.apiVersion('apps/v1').kind('Deployment').list()"
  validations:
    - expression: "variables.deploys.items.size() >= 1"
      message: "no deployments found"
"#;

    let bindings = serde_json::json!({ "object": { "kind": "Namespace" } });
    let logger = test_logger();
    let wasm_bytes = Builder::new()
        .with_logger(logger.clone())
        .build()
        .compile_vap(yaml)
        .unwrap();

    let result_str = runtime::Builder::new()
        .with_logger(logger)
        .with_log_level(LogLevel::Info)
        .with_wasm(wasm_bytes)
        .with_extension(make_kw_k8s_list_decl(), |args| {
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
        })
        .build()
        .unwrap()
        .eval(Some(&bindings.to_string()))
        .unwrap();

    let result: serde_json::Value = serde_json::from_str(&result_str).unwrap();
    assert_accepted(&result);
}

/// list() terminal — host returns empty list → validation fails.
#[test]
fn test_vap_kw_k8s_list_reject() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-list-reject
spec:
  variables:
    - name: deploys
      expression: "kw.k8s.apiVersion('apps/v1').kind('Deployment').list()"
  validations:
    - expression: "variables.deploys.items.size() >= 1"
      message: "no deployments found"
"#;

    let bindings = serde_json::json!({ "object": { "kind": "Namespace" } });
    let logger = test_logger();
    let wasm_bytes = Builder::new()
        .with_logger(logger.clone())
        .build()
        .compile_vap(yaml)
        .unwrap();

    let result_str = runtime::Builder::new()
        .with_logger(logger)
        .with_log_level(LogLevel::Info)
        .with_wasm(wasm_bytes)
        .with_extension(make_kw_k8s_list_decl(), |_args| {
            Ok(serde_json::json!({ "items": [] }))
        })
        .build()
        .unwrap()
        .eval(Some(&bindings.to_string()))
        .unwrap();

    let result: serde_json::Value = serde_json::from_str(&result_str).unwrap();
    assert_rejected(&result, Some("no deployments found"), Some(422));
}

/// .namespace() chain step is forwarded to the host inside the builder map.
#[test]
fn test_vap_kw_k8s_list_with_namespace() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-list-namespace
spec:
  variables:
    - name: deploys
      expression: "kw.k8s.apiVersion('apps/v1').kind('Deployment').namespace('prod').list()"
  validations:
    - expression: "variables.deploys.items.size() >= 1"
      message: "no prod deployments"
"#;

    let bindings = serde_json::json!({ "object": { "kind": "Namespace" } });
    let logger = test_logger();
    let wasm_bytes = Builder::new()
        .with_logger(logger.clone())
        .build()
        .compile_vap(yaml)
        .unwrap();

    let result_str = runtime::Builder::new()
        .with_logger(logger)
        .with_log_level(LogLevel::Info)
        .with_wasm(wasm_bytes)
        .with_extension(make_kw_k8s_list_decl(), |args| {
            let map = &args[0];
            assert_eq!(map["apiVersion"], "apps/v1");
            assert_eq!(map["kind"], "Deployment");
            assert_eq!(map["namespace"], "prod", "namespace not forwarded to host");
            Ok(serde_json::json!({
                "items": [{ "metadata": { "name": "prod-deploy", "namespace": "prod" } }]
            }))
        })
        .build()
        .unwrap()
        .eval(Some(&bindings.to_string()))
        .unwrap();

    let result: serde_json::Value = serde_json::from_str(&result_str).unwrap();
    assert_accepted(&result);
}

/// .labelSelector() chain step is forwarded to the host inside the builder map.
#[test]
fn test_vap_kw_k8s_list_with_label_selector() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-list-label-selector
spec:
  variables:
    - name: webDeploys
      expression: "kw.k8s.apiVersion('apps/v1').kind('Deployment').labelSelector('app=web').list()"
  validations:
    - expression: "variables.webDeploys.items.size() == 1"
      message: "expected exactly one web deployment"
"#;

    let bindings = serde_json::json!({ "object": { "kind": "Namespace" } });
    let logger = test_logger();
    let wasm_bytes = Builder::new()
        .with_logger(logger.clone())
        .build()
        .compile_vap(yaml)
        .unwrap();

    let result_str = runtime::Builder::new()
        .with_logger(logger)
        .with_log_level(LogLevel::Info)
        .with_wasm(wasm_bytes)
        .with_extension(make_kw_k8s_list_decl(), |args| {
            let map = &args[0];
            assert_eq!(
                map["labelSelector"], "app=web",
                "labelSelector not forwarded"
            );
            Ok(serde_json::json!({
                "items": [{ "metadata": { "name": "web-deploy" } }]
            }))
        })
        .build()
        .unwrap()
        .eval(Some(&bindings.to_string()))
        .unwrap();

    let result: serde_json::Value = serde_json::from_str(&result_str).unwrap();
    assert_accepted(&result);
}

/// .namespace() + .get() — both namespace and name reach the host; validation
/// reads a field from the returned resource.
#[test]
fn test_vap_kw_k8s_get_with_namespace() {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-get-namespace
spec:
  variables:
    - name: cfg
      expression: "kw.k8s.apiVersion('v1').kind('ConfigMap').namespace('default').get('my-config')"
  validations:
    - expression: "variables.cfg.data.key == 'expected-value'"
      message: "config key mismatch"
"#;

    let bindings = serde_json::json!({ "object": { "kind": "Deployment" } });
    let logger = test_logger();
    let wasm_bytes = Builder::new()
        .with_logger(logger.clone())
        .build()
        .compile_vap(yaml)
        .unwrap();

    let result_str = runtime::Builder::new()
        .with_logger(logger)
        .with_log_level(LogLevel::Info)
        .with_wasm(wasm_bytes)
        .with_extension(make_kw_k8s_get_decl(), |args| {
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
        })
        .build()
        .unwrap()
        .eval(Some(&bindings.to_string()))
        .unwrap();

    let result: serde_json::Value = serde_json::from_str(&result_str).unwrap();
    assert_accepted(&result);
}
