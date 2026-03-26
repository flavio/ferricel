// Integration tests for the ferricel-core compiler.
// These tests compile CEL expressions to WASM and execute them to verify correctness.
//
// Shared test helpers live in `common/mod.rs`. To add a new test file, create
// `tests/foo_tests.rs`, add `mod common; use common::*;` at the top, and move
// the relevant test functions there.

mod common;
use common::*;

use ferricel_core::compiler::{compile_cel_to_wasm, CompilerOptions};
use ferricel_core::runtime::CelEngine;
use ferricel_types::LogLevel;
use rstest::rstest;

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

    // WASM files start with magic number: 0x00 0x61 0x73 0x6D (\\0asm)
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

// ============================================================
// Container Resolution Tests
// ============================================================

#[test]
fn test_container_no_schema_no_container() {
    // Without schema or container, unqualified names should still work (treated as arbitrary structs)
    let result = compile_with_container("MyType{field: 42}", None, None);
    assert!(
        result.is_ok(),
        "Should compile unqualified struct name without schema"
    );
}

#[test]
fn test_container_with_container_but_no_schema() {
    // With container but no schema, resolution should fall back to using the name as-is
    // This is a graceful degradation case
    let result = compile_with_container("MyType{field: 42}", Some("com.example"), None);
    assert!(
        result.is_ok(),
        "Should compile with container but no schema (graceful degradation)"
    );
}

// Note: More comprehensive tests with proto descriptors would require building
// the proto files first with protoc. For now, these tests verify the basic
// container resolution logic compiles correctly.

// ============================================================
// Extension Function Tests
// ============================================================

#[test]
fn test_extension_global_call() {
    // Register myFunc(x) that doubles its argument, call myFunc(21) -> 42.
    let logger = create_test_logger();
    let decl = ExtensionDecl {
        namespace: None,
        function: "myFunc".to_string(),
        receiver_style: false,
        global_style: true,
        num_args: 1,
    };
    let mut engine = CelEngine::new(logger);
    engine.register_extension(decl.clone(), |args| {
        let x = args[0].as_i64().unwrap_or(0);
        Ok(serde_json::Value::Number((x * 2).into()))
    });

    let wasm = compile_cel_to_wasm("myFunc(21)", options_with_ext(decl)).expect("compile failed");
    let result = engine.execute(&wasm, None).expect("execute failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 42);
}

#[test]
fn test_extension_namespaced_call() {
    // Register math.abs(x), call math.abs(-7) -> 7.
    let logger = create_test_logger();
    let decl = ExtensionDecl {
        namespace: Some("math".to_string()),
        function: "abs".to_string(),
        receiver_style: false,
        global_style: true,
        num_args: 1,
    };
    let mut engine = CelEngine::new(logger);
    engine.register_extension(decl.clone(), |args| {
        let x = args[0].as_i64().unwrap_or(0);
        Ok(serde_json::Value::Number(x.abs().into()))
    });

    let wasm = compile_cel_to_wasm("math.abs(-7)", options_with_ext(decl)).expect("compile failed");
    let result = engine.execute(&wasm, None).expect("execute failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 7);
}

#[test]
fn test_extension_receiver_style_call() {
    // Register reverse(x) with receiver_style, call "hello".reverse() -> "olleh".
    let logger = create_test_logger();
    let decl = ExtensionDecl {
        namespace: None,
        function: "reverse".to_string(),
        receiver_style: true,
        global_style: false,
        num_args: 1,
    };
    let mut engine = CelEngine::new(logger);
    engine.register_extension(decl.clone(), |args| {
        let s = args[0]
            .as_str()
            .unwrap_or("")
            .chars()
            .rev()
            .collect::<String>();
        Ok(serde_json::Value::String(s))
    });

    let wasm = compile_cel_to_wasm(r#""hello".reverse()"#, options_with_ext(decl))
        .expect("compile failed");
    let result = engine.execute(&wasm, None).expect("execute failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_str().unwrap(), "olleh");
}

#[test]
fn test_extension_both_call_styles() {
    // Register reverse with both receiver and global style.
    // "hello".reverse() and reverse("hello") should both give "olleh".
    let logger = create_test_logger();
    let decl = ExtensionDecl {
        namespace: None,
        function: "rev".to_string(),
        receiver_style: true,
        global_style: true,
        num_args: 1,
    };
    let mut engine = CelEngine::new(logger);
    engine.register_extension(decl.clone(), |args| {
        let s = args[0]
            .as_str()
            .unwrap_or("")
            .chars()
            .rev()
            .collect::<String>();
        Ok(serde_json::Value::String(s))
    });

    let wasm_recv = compile_cel_to_wasm(r#""hello".rev()"#, options_with_ext(decl.clone()))
        .expect("compile receiver failed");
    let result_recv = engine
        .execute(&wasm_recv, None)
        .expect("execute receiver failed");
    let v_recv: serde_json::Value = serde_json::from_str(&result_recv).unwrap();

    let wasm_glob = compile_cel_to_wasm(r#"rev("hello")"#, options_with_ext(decl))
        .expect("compile global failed");
    let result_glob = engine
        .execute(&wasm_glob, None)
        .expect("execute global failed");
    let v_glob: serde_json::Value = serde_json::from_str(&result_glob).unwrap();

    assert_eq!(v_recv.as_str().unwrap(), "olleh");
    assert_eq!(v_glob.as_str().unwrap(), "olleh");
}

#[test]
fn test_extension_multi_arg() {
    // Register add3(a, b, c), call add3(1, 2, 3) -> 6.
    let (engine, decl) = make_engine_with_sum(None, "add3", 3, false, true);

    let wasm =
        compile_cel_to_wasm("add3(1, 2, 3)", options_with_ext(decl)).expect("compile failed");
    let result = engine.execute(&wasm, None).expect("execute failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 6);
}

#[test]
fn test_extension_arity_mismatch_is_compile_error() {
    // Register myFunc with num_args=1, try to compile myFunc(1, 2) -> error.
    let (_engine, decl) = make_engine_with_sum(None, "myFunc", 1, false, true);

    let result = compile_cel_to_wasm("myFunc(1, 2)", options_with_ext(decl));
    assert!(
        result.is_err(),
        "Arity mismatch should produce a compile error"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("expects") || msg.contains("argument"),
        "Error message should mention argument count, got: {msg}"
    );
}

#[test]
fn test_extension_unknown_function_is_compile_error() {
    // No extensions registered; calling unknown() should error.
    let options = CompilerOptions {
        proto_descriptor: None,
        container: None,
        logger: create_test_logger(),
        extensions: vec![],
    };

    let result = compile_cel_to_wasm("unknown(1)", options);
    assert!(
        result.is_err(),
        "Calling an unknown function should produce a compile error"
    );
}

#[test]
fn test_extension_wrong_call_style_is_compile_error() {
    // Register myFunc with global_style only; try receiver-style -> error.
    let (_engine, decl) = make_engine_with_sum(None, "myFunc", 1, false, true);

    let result = compile_cel_to_wasm("42.myFunc()", options_with_ext(decl));
    assert!(
        result.is_err(),
        "Using receiver-style on a global-only extension should error"
    );
}

#[test]
fn test_extension_with_bindings() {
    // math.abs(x) where x comes from bindings.
    let logger = create_test_logger();
    let decl = ExtensionDecl {
        namespace: Some("math".to_string()),
        function: "abs".to_string(),
        receiver_style: false,
        global_style: true,
        num_args: 1,
    };
    let mut engine = CelEngine::new(logger);
    engine.register_extension(decl.clone(), |args| {
        let x = args[0].as_i64().unwrap_or(0);
        Ok(serde_json::Value::Number(x.abs().into()))
    });

    let wasm =
        compile_cel_to_wasm("math.abs(input)", options_with_ext(decl)).expect("compile failed");
    let bindings = r#"{"input": -99}"#;
    let result = engine
        .execute(&wasm, Some(bindings))
        .expect("execute failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 99);
}
