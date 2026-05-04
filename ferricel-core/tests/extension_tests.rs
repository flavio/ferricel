// Integration tests for extension function registration and invocation.

use crate::common::*;

use ferricel_core::{compiler, runtime};
use ferricel_types::extensions::ExtensionDecl;

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
    let wasm = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl.clone())
        .build()
        .compile("myFunc(21)")
        .expect("compile failed");
    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(decl, |args| {
            let x = args[0].as_i64().unwrap_or(0);
            Ok(serde_json::Value::Number((x * 2).into()))
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");
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
    let wasm = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl.clone())
        .build()
        .compile("math.abs(-7)")
        .expect("compile failed");
    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(decl, |args| {
            let x = args[0].as_i64().unwrap_or(0);
            Ok(serde_json::Value::Number(x.abs().into()))
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");
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
    let wasm = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl.clone())
        .build()
        .compile(r#""hello".reverse()"#)
        .expect("compile failed");
    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(decl, |args| {
            let s = args[0]
                .as_str()
                .unwrap_or("")
                .chars()
                .rev()
                .collect::<String>();
            Ok(serde_json::Value::String(s))
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_str().unwrap(), "olleh");
}

#[test]
fn test_extension_both_call_styles() {
    // Register rev with both receiver and global style.
    // "hello".rev() and rev("hello") should both give "olleh".
    let logger = create_test_logger();
    let decl = ExtensionDecl {
        namespace: None,
        function: "rev".to_string(),
        receiver_style: true,
        global_style: true,
        num_args: 1,
    };

    let wasm_recv = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl.clone())
        .build()
        .compile(r#""hello".rev()"#)
        .expect("compile receiver failed");
    let result_recv = runtime::Builder::new()
        .with_logger(logger.clone())
        .with_extension(decl.clone(), |args| {
            let s = args[0]
                .as_str()
                .unwrap_or("")
                .chars()
                .rev()
                .collect::<String>();
            Ok(serde_json::Value::String(s))
        })
        .with_wasm(wasm_recv)
        .build()
        .expect("build receiver failed")
        .eval(None)
        .expect("eval receiver failed");

    let wasm_glob = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl.clone())
        .build()
        .compile(r#"rev("hello")"#)
        .expect("compile global failed");
    let result_glob = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(decl, |args| {
            let s = args[0]
                .as_str()
                .unwrap_or("")
                .chars()
                .rev()
                .collect::<String>();
            Ok(serde_json::Value::String(s))
        })
        .with_wasm(wasm_glob)
        .build()
        .expect("build global failed")
        .eval(None)
        .expect("eval global failed");

    let v_recv: serde_json::Value = serde_json::from_str(&result_recv).unwrap();
    let v_glob: serde_json::Value = serde_json::from_str(&result_glob).unwrap();
    assert_eq!(v_recv.as_str().unwrap(), "olleh");
    assert_eq!(v_glob.as_str().unwrap(), "olleh");
}

#[test]
fn test_extension_multi_arg() {
    // Register add3(a, b, c), call add3(1, 2, 3) -> 6.
    let logger = create_test_logger();
    let decl = ExtensionDecl {
        namespace: None,
        function: "add3".to_string(),
        receiver_style: false,
        global_style: true,
        num_args: 3,
    };
    let wasm = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl.clone())
        .build()
        .compile("add3(1, 2, 3)")
        .expect("compile failed");
    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(decl, |args| {
            let sum: i64 = args.iter().filter_map(|v| v.as_i64()).sum();
            Ok(serde_json::Value::Number(sum.into()))
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 6);
}

#[test]
fn test_extension_arity_mismatch_is_compile_error() {
    // Register myFunc with num_args=1, try to compile myFunc(1, 2) -> error.
    let decl = ExtensionDecl {
        namespace: None,
        function: "myFunc".to_string(),
        receiver_style: false,
        global_style: true,
        num_args: 1,
    };
    let result = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl)
        .build()
        .compile("myFunc(1, 2)");
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
fn test_extension_unknown_function_runtime_error() {
    // No extensions registered; calling unknown(1) compiles successfully but
    // produces a "no matching overload" error at runtime (CEL defers unknown
    // function errors to evaluation time).
    let logger = create_test_logger();
    let wasm = compiler::Builder::new()
        .with_logger(create_test_logger())
        .build()
        .compile("unknown(1)")
        .expect("compile should succeed — unknown functions are deferred to runtime");
    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None);
    assert!(
        result.is_err(),
        "Calling an unknown function should produce a runtime error"
    );
}

#[test]
fn test_extension_runtime_impl_without_compiler_decl_produces_no_matching_overload() {
    // Compile abs(x) WITHOUT declaring the extension to the compiler.
    // The compiler emits a deferred "no matching overload" error in the Wasm.
    let wasm = compiler::Builder::new()
        .build()
        .compile("abs(x)")
        .expect("compile should succeed — unknown functions are deferred to runtime");

    // Run WITH an implementation registered on the Engine.
    // The implementation is never reached because the compiler already
    // baked in the error; eval() should trap with "no matching overload".
    let abs_decl = ExtensionDecl {
        namespace: None,
        function: "abs".to_string(),
        global_style: true,
        receiver_style: false,
        num_args: 1,
    };

    let result = runtime::Builder::new()
        .with_wasm(wasm)
        .with_extension(abs_decl, |args| {
            let n = args[0].as_i64().unwrap_or(0);
            Ok(serde_json::Value::Number(n.abs().into()))
        })
        .build()
        .expect("build failed")
        .eval(Some(r#"{"x": -5}"#));

    assert!(
        result.is_err(),
        "should produce a runtime error, not a value"
    );
    let msg = format!("{:#}", result.unwrap_err());
    assert!(
        msg.contains("no matching overload"),
        "expected 'no matching overload', got: {msg}"
    );
}

#[test]
fn test_extension_wrong_call_style_is_compile_error() {
    // Register myFunc with global_style only; try receiver-style -> error.
    let decl = ExtensionDecl {
        namespace: None,
        function: "myFunc".to_string(),
        receiver_style: false,
        global_style: true,
        num_args: 1,
    };
    let result = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl)
        .build()
        .compile("42.myFunc()");
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
    let wasm = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl.clone())
        .build()
        .compile("math.abs(input)")
        .expect("compile failed");
    let bindings = r#"{"input": -99}"#;
    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(decl, |args| {
            let x = args[0].as_i64().unwrap_or(0);
            Ok(serde_json::Value::Number(x.abs().into()))
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(Some(bindings))
        .expect("eval failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 99);
}
