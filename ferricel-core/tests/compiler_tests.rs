use ferricel_core::compiler::Builder;

use crate::common::*;

#[test]
fn test_compile_cel_to_wasm_returns_valid_bytes() {
    let logger = create_test_logger();
    let wasm_bytes = Builder::new()
        .with_logger(logger)
        .build()
        .compile("42")
        .expect("Failed to compile");
    assert!(!wasm_bytes.is_empty(), "Wasm bytes should not be empty");

    // Wasm files start with magic number: 0x00 0x61 0x73 0x6D (\0asm)
    assert_eq!(
        &wasm_bytes[0..4],
        &[0x00, 0x61, 0x73, 0x6D],
        "Should have Wasm magic number"
    );
}

#[test]
fn test_invalid_cel_expression() {
    let logger = create_test_logger();
    let result = Builder::new()
        .with_logger(logger)
        .build()
        .compile("1 + + 2");
    assert!(
        result.is_err(),
        "Invalid CEL expression should return error"
    );
}

#[test]
fn test_producers_custom_section_present() {
    let wasm_bytes = Builder::new()
        .build()
        .compile("42")
        .expect("Failed to compile");

    // The producers section is encoded as a WebAssembly custom section whose
    // field names and versioned-name strings appear verbatim as LEB128-prefixed
    // UTF-8 in the binary.  Searching for the raw bytes of each expected string
    // is sufficient to verify the section content without depending on an
    // external parser or walrus internals.
    let contains = |needle: &[u8]| wasm_bytes.windows(needle.len()).any(|w| w == needle);

    assert!(contains(b"producers"), "producers section name not found");
    assert!(contains(b"language"), "'language' field not found");
    assert!(contains(b"CEL"), "'CEL' language entry not found");
    assert!(contains(b"processed-by"), "'processed-by' field not found");
    assert!(contains(b"ferricel"), "'ferricel' tool entry not found");

    let version = env!("CARGO_PKG_VERSION").as_bytes();
    assert!(
        contains(version),
        "ferricel version '{}' not found in producers section",
        env!("CARGO_PKG_VERSION"),
    );
}

#[test]
fn test_cel_source_custom_section_present() {
    let expression = "x + y * 2";
    let wasm_bytes = Builder::new()
        .build()
        .compile(expression)
        .expect("Failed to compile");

    // The ferricel.cel-source custom section stores the original CEL expression as raw
    // UTF-8.  The section name and content both appear verbatim in the binary.
    let contains = |needle: &[u8]| wasm_bytes.windows(needle.len()).any(|w| w == needle);

    assert!(contains(b"ferricel.cel-source"), "ferricel.cel-source section name not found");
    assert!(
        contains(expression.as_bytes()),
        "original CEL expression not found in ferricel.cel-source section"
    );
    assert!(
        !contains(b"ferricel.vap-source"),
        "ferricel.vap-source should not be present in a plain CEL compilation"
    );
}

#[test]
fn test_vap_source_custom_section_present() {
    let vap_yaml = r#"
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingAdmissionPolicy
metadata:
  name: test-source-section
spec:
  validations:
    - expression: "object.spec.replicas <= 5"
      message: "replicas must be 5 or less"
"#;
    let wasm_bytes = Builder::new()
        .build()
        .compile_vap(vap_yaml)
        .expect("Failed to compile VAP");

    // The ferricel.vap-source custom section stores the full ValidatingAdmissionPolicy
    // serialized back to YAML. The section name appears verbatim; key field values
    // must survive the round-trip even if formatting differs from the original input.
    let contains = |needle: &[u8]| wasm_bytes.windows(needle.len()).any(|w| w == needle);

    assert!(
        contains(b"ferricel.vap-source"),
        "ferricel.vap-source section name not found"
    );
    assert!(
        contains(b"ValidatingAdmissionPolicy"),
        "kind not found in ferricel.vap-source section"
    );
    assert!(
        contains(b"admissionregistration.k8s.io"),
        "apiVersion not found in ferricel.vap-source section"
    );
    assert!(
        contains(b"object.spec.replicas <= 5"),
        "CEL expression not found in ferricel.vap-source section"
    );
    assert!(
        contains(b"replicas must be 5 or less"),
        "validation message not found in ferricel.vap-source section"
    );
    assert!(
        !contains(b"ferricel.cel-source"),
        "ferricel.cel-source should not be present in a VAP compilation"
    );
}
