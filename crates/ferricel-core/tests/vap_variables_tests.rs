// Tests for the ferricel.vap-variables custom section.
//
// Verifies that vap_variables_used() correctly reads back the set of
// well-known VAP variables (a subset of WELL_KNOWN_VAP_VARIABLES) referenced
// by a compiled VAP module, and that plain CEL modules (compile()) never
// carry this section at all.

#![cfg(feature = "k8s-vap")]

use ferricel_core::{compiler::Builder, vap_variables_used};
use rstest::rstest;

use crate::common::*;

const VAP_OBJECT_ONLY: &str = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test
spec:
  validations:
    - expression: "object.spec.replicas <= 5"
      message: "too many replicas"
"#;

const VAP_NAMESPACE_OBJECT_IN_VALIDATION: &str = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-ns
spec:
  validations:
    - expression: "object.metadata.namespace == namespaceObject.metadata.name"
      message: "namespace mismatch"
"#;

const VAP_NAMESPACE_OBJECT_IN_MESSAGE_EXPRESSION: &str = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-ns-msg
spec:
  validations:
    - expression: "object.spec.replicas <= 5"
      messageExpression: "'namespace ' + namespaceObject.metadata.name + ' has too many replicas'"
"#;

const VAP_NAMESPACE_OBJECT_IN_VARIABLE: &str = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-ns-var
spec:
  variables:
    - name: environment
      expression: "'environment' in namespaceObject.metadata.labels ? namespaceObject.metadata.labels['environment'] : 'prod'"
  validations:
    - expression: "variables.environment == 'prod'"
      message: "not prod"
"#;

const VAP_NAMESPACE_OBJECT_SHADOWED: &str = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-ns-shadowed
spec:
  validations:
    - expression: "object.spec.tags.exists(namespaceObject, namespaceObject == 'prod')"
      message: "no prod tag"
"#;

const VAP_OLD_OBJECT_AND_REQUEST: &str = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-old-req
spec:
  validations:
    - expression: "oldObject == null || request.operation == 'UPDATE'"
      message: "invalid transition"
"#;

const VAP_PARAM_KIND: &str = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-params
spec:
  paramKind:
    apiVersion: v1
    kind: ConfigMap
  validations:
    - expression: "object.spec.replicas <= int(params.data.maxreplicas)"
      message: "too many replicas"
"#;

#[rstest]
// Plain `object` usage only — nothing else recorded.
#[case::object_only(VAP_OBJECT_ONLY, vec!["object"])]
// `namespaceObject` referenced (via dotted select chain) inside a validation expression.
#[case::namespace_object_validation(
    VAP_NAMESPACE_OBJECT_IN_VALIDATION,
    vec!["namespaceObject", "object"]
)]
// `namespaceObject` referenced inside a messageExpression.
#[case::namespace_object_message_expression(
    VAP_NAMESPACE_OBJECT_IN_MESSAGE_EXPRESSION,
    vec!["namespaceObject", "object"]
)]
// `namespaceObject` referenced inside a spec.variables[] definition.
#[case::namespace_object_variable(VAP_NAMESPACE_OBJECT_IN_VARIABLE, vec!["namespaceObject"])]
// A comprehension-bound local named `namespaceObject` shadows the real
// variable — it must NOT be recorded as a referenced VAP variable.
#[case::namespace_object_shadowed_by_comprehension(
    VAP_NAMESPACE_OBJECT_SHADOWED,
    vec!["object"]
)]
// `oldObject` and `request` are tracked too, not just `namespaceObject`.
#[case::old_object_and_request(VAP_OLD_OBJECT_AND_REQUEST, vec!["oldObject", "request"])]
// `params` is tracked, and the internal `paramRef` extension plumbing does not
// leak unrelated names into the recorded set.
#[case::params(VAP_PARAM_KIND, vec!["object", "params"])]
fn test_vap_variables_used(#[case] yaml: &str, #[case] expected: Vec<&str>) {
    let wasm = Builder::new()
        .with_logger(create_test_logger())
        .build()
        .compile_vap(yaml)
        .expect("compile_vap failed");

    let expected: Vec<String> = expected.into_iter().map(String::from).collect();
    assert_eq!(vap_variables_used(&wasm).expect("reader failed"), expected);
}

/// `variables` (the internal VAP variables map) and other non-well-known
/// identifiers are never recorded, even though they are real runtime
/// variable lookups.
#[test]
fn test_vap_variables_excludes_internal_variables_map() {
    let wasm = Builder::new()
        .with_logger(create_test_logger())
        .build()
        .compile_vap(VAP_NAMESPACE_OBJECT_IN_VARIABLE)
        .expect("compile_vap failed");

    let used = vap_variables_used(&wasm).expect("reader failed");
    assert!(!used.iter().any(|v| v == "variables"));
}

/// Plain CEL modules (compile(), not compile_vap()) never carry the
/// ferricel.vap-variables section, even when the expression happens to
/// reference an identifier with the same name as a well-known VAP variable.
#[test]
fn test_plain_cel_module_has_no_vap_variables_section() {
    let wasm = Builder::new()
        .with_logger(create_test_logger())
        .build()
        .compile("namespaceObject.metadata.name == 'default'")
        .expect("compile failed");

    let used = vap_variables_used(&wasm).expect("reader failed");
    assert!(used.is_empty());

    let info = ferricel_core::inspect(&wasm).expect("inspect failed");
    assert!(info.vap_variables.is_empty());
}
