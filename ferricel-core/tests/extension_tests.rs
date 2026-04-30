// Integration tests for extension function registration and invocation.

mod common;
use common::*;

use ferricel_core::runtime::CelEngine;

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

    let wasm = options_with_ext(decl)
        .compile("myFunc(21)")
        .expect("compile failed");
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

    let wasm = options_with_ext(decl)
        .compile("math.abs(-7)")
        .expect("compile failed");
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

    let wasm = options_with_ext(decl)
        .compile(r#""hello".reverse()"#)
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

    let wasm_recv = options_with_ext(decl.clone())
        .compile(r#""hello".rev()"#)
        .expect("compile receiver failed");
    let result_recv = engine
        .execute(&wasm_recv, None)
        .expect("execute receiver failed");
    let v_recv: serde_json::Value = serde_json::from_str(&result_recv).unwrap();

    let wasm_glob = options_with_ext(decl)
        .compile(r#"rev("hello")"#)
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

    let wasm = options_with_ext(decl)
        .compile("add3(1, 2, 3)")
        .expect("compile failed");
    let result = engine.execute(&wasm, None).expect("execute failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 6);
}

#[test]
fn test_extension_arity_mismatch_is_compile_error() {
    // Register myFunc with num_args=1, try to compile myFunc(1, 2) -> error.
    let (_engine, decl) = make_engine_with_sum(None, "myFunc", 1, false, true);

    let result = options_with_ext(decl).compile("myFunc(1, 2)");
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
    let result = ferricel_core::compiler::Builder::new()
        .with_logger(create_test_logger())
        .build()
        .compile("unknown(1)");
    assert!(
        result.is_err(),
        "Calling an unknown function should produce a compile error"
    );
}

#[test]
fn test_extension_wrong_call_style_is_compile_error() {
    // Register myFunc with global_style only; try receiver-style -> error.
    let (_engine, decl) = make_engine_with_sum(None, "myFunc", 1, false, true);

    let result = options_with_ext(decl).compile("42.myFunc()");
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

    let wasm = options_with_ext(decl)
        .compile("math.abs(input)")
        .expect("compile failed");
    let bindings = r#"{"input": -99}"#;
    let result = engine
        .execute(&wasm, Some(bindings))
        .expect("execute failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 99);
}
