// Integration tests for JSON serialization of CEL evaluation results.

mod common;
use common::*;

use ferricel_core::compiler::{CompilerOptions, compile_cel_to_wasm};
use ferricel_core::runtime::CelEngine;
use ferricel_types::LogLevel;

#[test]
fn test_json_output_integer() {
    // Test that integers are serialized as raw JSON numbers
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm("42", compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(
        json_result, "42",
        "Integer should be serialized as raw JSON number"
    );
}

#[test]
fn test_json_output_boolean_true() {
    // Test that true is serialized as raw JSON boolean
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm("5 > 3", compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(
        json_result, "true",
        "Boolean true should be serialized as 'true'"
    );
}

#[test]
fn test_json_output_boolean_false() {
    // Test that false is serialized as raw JSON boolean
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm("5 < 3", compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(
        json_result, "false",
        "Boolean false should be serialized as 'false'"
    );
}

#[test]
fn test_json_output_negative_integer() {
    // Test that negative integers are properly serialized
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes = compile_cel_to_wasm("-123", compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(
        json_result, "-123",
        "Negative integer should be serialized correctly"
    );
}

#[test]
fn test_json_output_arithmetic_result() {
    // Test that arithmetic results are serialized correctly
    let logger = create_test_logger();
    let compiler_options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: logger.clone(),
        extensions: vec![],
    };
    let wasm_bytes =
        compile_cel_to_wasm("10 + 20 * 2", compiler_options).expect("Failed to compile");
    let json_result = CelEngine::new(logger)
        .with_log_level(LogLevel::Info)
        .execute(&wasm_bytes, None)
        .expect("Failed to execute");
    assert_eq!(
        json_result, "50",
        "Arithmetic result should be serialized correctly"
    );
}
