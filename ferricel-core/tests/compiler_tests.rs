use crate::common::*;

use ferricel_core::compiler::Builder;

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
