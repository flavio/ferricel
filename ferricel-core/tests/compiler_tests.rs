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
