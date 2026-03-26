// Integration tests for the ferricel-core compiler.
// These tests compile CEL expressions to WASM and execute them to verify correctness.
//
// Shared test helpers live in `common/mod.rs`. To add a new test file, create
// `tests/foo_tests.rs`, add `mod common; use common::*;` at the top, and move
// the relevant test functions there.

mod common;
use common::*;

use ferricel_core::compiler::{compile_cel_to_wasm, CompilerOptions};

#[test]
fn test_compile_cel_to_wasm_returns_valid_bytes() {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger,
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm("42", compiler_options).expect("Failed to compile");
    assert!(!wasm_bytes.is_empty(), "WASM bytes should not be empty");

    // WASM files start with magic number: 0x00 0x61 0x73 0x6D (\0asm)
    assert_eq!(
        &wasm_bytes[0..4],
        &[0x00, 0x61, 0x73, 0x6D],
        "Should have WASM magic number"
    );
}

#[test]
fn test_invalid_cel_expression() {
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger,
        extensions: vec![],
    };
    let result = compile_cel_to_wasm("1 + + 2", compiler_options);
    assert!(
        result.is_err(),
        "Invalid CEL expression should return error"
    );
}
