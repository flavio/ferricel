// Tests for the `ferricel.abi-version` custom section.
//
// Verifies that the compiler embeds the section, that `abi_version()` and
// `inspect()` read it back, and that `runtime::Builder::build_pre` rejects a
// module whose section is missing, wrong, or unreadable.

use ferricel_core::{ABI_VERSION, abi_version, compiler, inspect, runtime};
use ferricel_types::ABI_VERSION_SECTION;
use rstest::rstest;

use crate::common::*;

/// Compile a VAP policy with a single, always-passing validation.
#[cfg(feature = "k8s-vap")]
fn vap_module() -> Vec<u8> {
    let yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test
spec:
  validations:
    - expression: "true"
"#;
    compiler::Builder::new()
        .with_logger(create_test_logger())
        .build()
        .compile_vap(yaml)
        .expect("compile_vap failed")
}

/// Re-emit `wasm` with the `ferricel.abi-version` section replaced by
/// `content` (or removed, when `content` is `None`).
fn rewrite_abi_version_section(wasm: &[u8], content: Option<&[u8]>) -> Vec<u8> {
    let mut module = walrus::ModuleConfig::new()
        .parse(wasm)
        .expect("failed to parse compiled Wasm module");

    module.customs.remove_raw(ABI_VERSION_SECTION);
    if let Some(content) = content {
        module.customs.add(walrus::RawCustomSection {
            name: ABI_VERSION_SECTION.to_string(),
            data: content.to_vec(),
        });
    }

    module.emit_wasm()
}

// ─── The compiler embeds a matching section ────────────────────────────────

#[rstest]
#[case::plain_cel(compile_with_container("1 + 1", None, None).expect("compile failed"))]
#[cfg_attr(feature = "k8s-vap", case::vap(vap_module()))]
fn test_compile_embeds_the_current_abi_version(#[case] wasm: Vec<u8>) {
    assert_eq!(
        abi_version(&wasm).expect("reader failed"),
        Some(ABI_VERSION)
    );
}

// ─── build_pre accepts a matching section ──────────────────────────────────

#[test]
fn test_build_pre_accepts_a_matching_abi_version() {
    runtime::Builder::new()
        .with_wasm(compile_with_container("1 + 1", None, None).expect("compile failed"))
        .build_pre()
        .expect("build_pre must accept a module with the current ABI version");
}

// ─── build_pre rejects a missing, mismatched, or unparsable section ───────

#[rstest]
#[case::missing_section(
    None,
    vec!["no ferricel.abi-version section".to_string(), "compiled by ferricel".to_string()]
)]
#[case::mismatched_version(
    Some((ABI_VERSION + 1).to_string()),
    vec![(ABI_VERSION + 1).to_string(), ABI_VERSION.to_string()]
)]
#[case::unparsable(
    Some("not-a-number".to_string()),
    vec!["not a valid number".to_string()]
)]
fn test_build_pre_rejects_a_bad_abi_version_section(
    #[case] section: Option<String>,
    #[case] expected_fragments: Vec<String>,
) {
    let wasm = rewrite_abi_version_section(
        &compile_with_container("1 + 1", None, None).expect("compile failed"),
        section.as_deref().map(str::as_bytes),
    );

    let Err(err) = runtime::Builder::new().with_wasm(wasm).build_pre() else {
        panic!("build_pre must reject this module");
    };

    let msg = err.to_string();
    for fragment in &expected_fragments {
        assert!(
            msg.contains(fragment.as_str()),
            "expected {fragment:?} in: {msg}"
        );
    }
}

// ─── The readers reject an unparsable section too ─────────────────────────

#[test]
fn test_readers_reject_an_unparsable_section() {
    let wasm = rewrite_abi_version_section(
        &compile_with_container("1 + 1", None, None).expect("compile failed"),
        Some(b"not-a-number"),
    );

    let err = abi_version(&wasm).expect_err("abi_version() must reject invalid content");
    assert!(err.to_string().contains("not a valid number"), "got: {err}");

    let err = inspect(&wasm).expect_err("inspect() must reject invalid content");
    assert!(err.to_string().contains("not a valid number"), "got: {err}");
}

// ─── inspect() reports None, not an error, for a module with no section ────

#[test]
fn test_inspect_reports_none_for_a_module_with_no_section() {
    let wasm = rewrite_abi_version_section(
        &compile_with_container("1 + 1", None, None).expect("compile failed"),
        None,
    );

    let info = inspect(&wasm).expect("inspect must still succeed");
    assert_eq!(info.abi_version, None);
}
