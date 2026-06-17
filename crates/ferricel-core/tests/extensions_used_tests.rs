// Tests for the ferricel.extensions custom section.
//
// Verifies that extensions_used() correctly reads back the set of host
// extensions emitted into a compiled Wasm module.

use ferricel_core::{UsedExtension, compiler, extensions_used};
use ferricel_types::extensions::{BuilderChainDecl, BuilderStep, ExtensionDecl};
use rstest::rstest;

use crate::common::*;

fn used(namespace: Option<&str>, function: &str) -> UsedExtension {
    UsedExtension {
        namespace: namespace.map(|s| s.to_string()),
        function: function.to_string(),
    }
}

fn flat_decl(namespace: Option<&str>, function: &str) -> ExtensionDecl {
    ExtensionDecl {
        namespace: namespace.map(String::from),
        function: function.to_string(),
        receiver_style: false,
        global_style: true,
        num_args: 1,
    }
}

// ─── Flat extensions ──────────────────────────────────────────────────────────

#[rstest]
// No extensions at all.
#[case::empty(vec![], "1 + 1", vec![])]
// Single non-namespaced extension.
#[case::single(vec![flat_decl(None, "myFunc")], "myFunc(x)", vec![used(None, "myFunc")])]
// Dotted multi-segment namespace.
#[case::namespaced(
    vec![flat_decl(Some("kw.net"), "lookupHost")],
    "kw.net.lookupHost('example.com')",
    vec![used(Some("kw.net"), "lookupHost")]
)]
// Repeated calls to the same extension deduplicates to one entry.
#[case::dedup(vec![flat_decl(None, "abs")], "abs(x) + abs(y)", vec![used(None, "abs")])]
// Two extensions appear sorted by (namespace, function): None < Some.
#[case::sorted(
    vec![flat_decl(Some("kw.net"), "lookupHost"), flat_decl(None, "abs")],
    "abs(x) + size(kw.net.lookupHost('h'))",
    vec![used(None, "abs"), used(Some("kw.net"), "lookupHost")]
)]
// CEL && does not short-circuit at compile time: ext() is still emitted and recorded.
#[case::short_circuit(
    vec![flat_decl(None, "ext")],
    "false && ext(x)",
    vec![used(None, "ext")]
)]
fn test_flat_extensions(
    #[case] decls: Vec<ExtensionDecl>,
    #[case] expr: &str,
    #[case] expected: Vec<UsedExtension>,
) {
    let mut builder = compiler::Builder::new().with_logger(create_test_logger());
    for d in decls {
        builder = builder.with_extension(d);
    }
    let wasm = builder.build().compile(expr).expect("compile failed");
    assert_eq!(extensions_used(&wasm).expect("reader failed"), expected);
}

// ─── Builder chain terminals ──────────────────────────────────────────────────

#[test]
fn test_extensions_used_builder_terminal_recorded() {
    let chain = BuilderChainDecl {
        steps: vec![
            BuilderStep::Entry {
                function: "q.start".to_string(),
                state_keys: vec!["val".to_string()],
                output_type: "q.Builder".to_string(),
            },
            // Intermediate Chain step — must NOT appear in the output.
            BuilderStep::Chain {
                function: "filter".to_string(),
                input_type: "q.Builder".to_string(),
                state_keys: vec!["filter".to_string()],
                output_type: "q.Builder".to_string(),
                accumulate: false,
            },
            BuilderStep::Terminal {
                function: "run".to_string(),
                input_type: "q.Builder".to_string(),
                extra_arg_keys: vec![],
                host_namespace: "q".to_string(),
                host_function: "run".to_string(),
            },
        ],
    };

    let wasm = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_builder_chain(chain)
        .build()
        .compile("q.start('x').filter('active').run()")
        .expect("compile failed");

    // Only the terminal host_namespace/host_function — intermediate steps are not host calls.
    assert_eq!(
        extensions_used(&wasm).expect("reader failed"),
        vec![used(Some("q"), "run")]
    );
}

// ─── VAP ─────────────────────────────────────────────────────────────────────

#[cfg(feature = "k8s-vap")]
mod vap {
    use ferricel_core::compiler::Builder;

    use super::*;

    const VAP_NO_EXT: &str = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test
spec:
  validations:
    - expression: "object.spec.replicas <= 5"
      message: "too many replicas"
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

    const VAP_LIST: &str = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-list
spec:
  variables:
    - name: pods
      expression: "kw.k8s.apiVersion('v1').kind('Pod').list()"
  validations:
    - expression: "variables.pods.items.size() >= 1"
      message: "no pods"
"#;

    #[rstest]
    #[case::no_ext(VAP_NO_EXT, vec![])]
    #[case::param_kind(VAP_PARAM_KIND, vec![used(Some("kw.k8s"), "get")])]
    #[case::list(VAP_LIST, vec![used(Some("kw.k8s"), "list")])]
    fn test_vap_extensions(#[case] yaml: &str, #[case] expected: Vec<UsedExtension>) {
        let wasm = Builder::new()
            .with_logger(create_test_logger())
            .build()
            .compile_vap(yaml)
            .expect("compile_vap failed");
        assert_eq!(extensions_used(&wasm).expect("reader failed"), expected);
    }
}
