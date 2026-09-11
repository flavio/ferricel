// Integration tests for extension function registration and invocation.

use std::sync::atomic::{AtomicBool, Ordering};

use ferricel_core::{CelRuntimeError, ExtensionOrigin, compiler, runtime, runtime::Extensions};
use ferricel_types::extensions::ExtensionDecl;

use crate::common::*;

// ============================================================
// Shared helpers
// ============================================================

/// A flat, global-style extension declaration with no namespace.
fn global_decl(function: &str, num_args: usize) -> ExtensionDecl {
    ExtensionDecl {
        namespace: None,
        function: function.to_string(),
        receiver_style: false,
        global_style: true,
        num_args,
    }
}

/// A flat, global-style extension declaration under `namespace`.
fn namespaced_decl(namespace: &str, function: &str, num_args: usize) -> ExtensionDecl {
    ExtensionDecl {
        namespace: Some(namespace.to_string()),
        function: function.to_string(),
        receiver_style: false,
        global_style: true,
        num_args,
    }
}

/// Compile `expr` with `decl` registered on the compiler.
fn compile_with_extension(decl: &ExtensionDecl, expr: &str) -> Vec<u8> {
    compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl.clone())
        .build()
        .compile(expr)
        .expect("compile failed")
}

/// Compile `expr` with `decl` registered on the compiler, and return the raw
/// `Result`. Use this to test a compile-time error.
fn try_compile_with_extension(decl: ExtensionDecl, expr: &str) -> Result<Vec<u8>, anyhow::Error> {
    compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl)
        .build()
        .compile(expr)
}

/// Compile `expr` with no extensions registered. CEL defers an unknown
/// function to runtime, so this still succeeds.
fn compile_without_extension(expr: &str) -> Vec<u8> {
    compiler::Builder::new()
        .with_logger(create_test_logger())
        .build()
        .compile(expr)
        .expect("compile should succeed — unknown functions are deferred to runtime")
}

// ============================================================
// Extension Function Tests
// ============================================================

#[test]
fn test_extension_global_call() {
    // Register myFunc(x) that doubles its argument, call myFunc(21) -> 42.
    let logger = create_test_logger();
    let decl = global_decl("myFunc", 1);
    let wasm = compile_with_extension(&decl, "myFunc(21)");
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
    let decl = namespaced_decl("math", "abs", 1);
    let wasm = compile_with_extension(&decl, "math.abs(-7)");
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
    let wasm = compile_with_extension(&decl, r#""hello".reverse()"#);
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

    let wasm_recv = compile_with_extension(&decl, r#""hello".rev()"#);
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

    let wasm_glob = compile_with_extension(&decl, r#"rev("hello")"#);
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
    let decl = global_decl("add3", 3);
    let wasm = compile_with_extension(&decl, "add3(1, 2, 3)");
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
    let decl = global_decl("myFunc", 1);
    let result = try_compile_with_extension(decl, "myFunc(1, 2)");
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
    let wasm = compile_without_extension("unknown(1)");
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
    let wasm = compile_without_extension("abs(x)");

    // Run WITH an implementation registered on the Engine.
    // The implementation is never reached because the compiler already
    // baked in the error; eval() should trap with "no matching overload".
    let abs_decl = global_decl("abs", 1);

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
    let decl = global_decl("myFunc", 1);
    let result = try_compile_with_extension(decl, "42.myFunc()");
    assert!(
        result.is_err(),
        "Using receiver-style on a global-only extension should error"
    );
}

#[test]
fn test_extension_with_bindings() {
    // math.abs(x) where x comes from bindings.
    let logger = create_test_logger();
    let decl = namespaced_decl("math", "abs", 1);
    let wasm = compile_with_extension(&decl, "math.abs(input)");
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

#[test]
fn test_extension_size_on_returned_array() {
    // Extension returns a bare JSON array; size() should return its length.
    let logger = create_test_logger();
    let decl = global_decl("getItems", 1);
    let wasm = compile_with_extension(&decl, "size(getItems('x'))");
    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(decl, |_args| Ok(serde_json::json!(["a", "b", "c"])))
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 3);
}

#[test]
fn test_extension_size_on_returned_array_comparison() {
    // Extension returns a bare JSON array; size(...) >= 1 should be true.
    let logger = create_test_logger();
    let decl = namespaced_decl("kw.net", "lookupHost", 1);
    let wasm = compile_with_extension(&decl, "size(kw.net.lookupHost('example.com')) >= 1");
    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(decl, |_args| Ok(serde_json::json!(["1.1.1.1", "8.8.8.8"])))
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(value.as_bool().unwrap());
}

#[test]
fn test_extension_size_on_returned_map() {
    // Extension returns a JSON object; size() should return the number of keys.
    let logger = create_test_logger();
    let decl = global_decl("getMap", 1);
    let wasm = compile_with_extension(&decl, "size(getMap('x'))");
    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(decl, |_args| Ok(serde_json::json!({"a": 1, "b": 2})))
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 2);
}

#[test]
fn test_extension_index_into_returned_array() {
    // Extension returns a bare JSON array; indexing [0] should work.
    let logger = create_test_logger();
    let decl = global_decl("getItems", 1);
    let wasm = compile_with_extension(&decl, r#"getItems('x')[0]"#);
    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(decl, |_args| Ok(serde_json::json!(["first", "second"])))
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_str().unwrap(), "first");
}

// ============================================================
// Argument count checks at runtime dispatch
// ============================================================

#[test]
fn test_extension_runtime_arity_mismatch_is_evaluation_error() {
    // Compile with a declaration that expects 2 arguments, so the Wasm
    // module sends 2 arguments. Register the runtime implementation with a
    // declaration that expects 1 argument. This models a Wasm module that
    // sends the wrong count. The runtime must reject the call and must not
    // call the closure.
    let compile_decl = global_decl("myFunc", 2);
    let wasm = compile_with_extension(&compile_decl, "myFunc(1, 2)");

    let runtime_decl = global_decl("myFunc", 1);
    let called = std::sync::Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let result = runtime::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(runtime_decl, move |args| {
            called_clone.store(true, Ordering::SeqCst);
            Ok(serde_json::Value::Number(args.len().into()))
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None);

    assert!(
        !called.load(Ordering::SeqCst),
        "closure must not be invoked"
    );
    assert!(
        result.is_err(),
        "arity mismatch should be an evaluation error"
    );
    let msg = format!("{:#}", result.unwrap_err());
    assert!(
        msg.contains("expects 1 argument(s), got 2"),
        "expected an arity mismatch message, got: {msg}"
    );
}

#[test]
fn test_extension_err_becomes_cel_evaluation_error() {
    // If the closure returns `Err(_)`, `Engine::eval` must return `Err`. It
    // must not return an `{"error": ...}` value.
    let decl = global_decl("failing", 0);
    let wasm = compile_with_extension(&decl, "failing()");
    let result = runtime::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl, |_args| Err("boom".to_string()))
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None);
    let err = result.expect_err("expected a runtime error");
    let msg = format!("{err:#}");
    assert!(msg.contains("boom"), "expected 'boom', got: {msg}");

    // The error downcasts to `CelRuntimeError` and records the extension
    // that produced it.
    let cel_err = err
        .downcast_ref::<CelRuntimeError>()
        .expect("error must downcast to CelRuntimeError");
    assert_eq!(cel_err.message, "boom");
    assert_eq!(
        cel_err.origin,
        Some(ExtensionOrigin {
            namespace: None,
            function: "failing".to_string(),
        })
    );
}

#[test]
fn test_extension_error_is_absorbed_by_logical_or() {
    // `failing() || true` must evaluate to `true`. The `||` operator absorbs
    // the error like any other CEL runtime error.
    let decl = global_decl("failing", 0);
    let wasm = compile_with_extension(&decl, "failing() || true");
    let result = runtime::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl, |_args| Err("boom".to_string()))
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("error should be absorbed by ||");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_bool(), Some(true));
}

#[test]
fn test_extension_ok_value_with_error_key_is_a_map() {
    // An extension can return a JSON object with an "error" key. This result
    // is a normal map value, not a CEL error.
    let decl = global_decl("getResult", 0);
    let wasm = compile_with_extension(&decl, "getResult()");
    let result = runtime::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl, |_args| Ok(serde_json::json!({"error": "not found"})))
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value["error"].as_str(), Some("not found"));
}

#[test]
fn test_extension_error_argument_short_circuits_before_host_call() {
    // In `ext(1 / 0)`, the argument is a `CelValue::Error` before the call.
    // The guest must return that error and must not call the host.
    let decl = global_decl("ext", 1);
    let wasm = compile_with_extension(&decl, "ext(1 / 0)");
    let called = std::sync::Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let result = runtime::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl, move |_args| {
            called_clone.store(true, Ordering::SeqCst);
            Ok(serde_json::Value::Null)
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None);
    assert!(
        !called.load(Ordering::SeqCst),
        "closure must not be invoked"
    );
    let err = result.expect_err("expected a runtime error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("divide by zero"),
        "expected 'divide by zero', got: {msg}"
    );
    // The error came from `1 / 0`, not from the extension. It has no origin.
    let cel_err = err
        .downcast_ref::<CelRuntimeError>()
        .expect("error must downcast to CelRuntimeError");
    assert_eq!(cel_err.origin, None);
}

#[test]
fn test_extension_error_message_with_quotes_and_newlines_survives_roundtrip() {
    // The error envelope is valid JSON. A message with quotes, backslashes,
    // and newlines must stay the same from the host to the guest and back.
    let decl = global_decl("failing", 0);
    let tricky_message = "a \"quoted\" \\ value\nwith a newline";
    let wasm = compile_with_extension(&decl, "failing()");
    let result = runtime::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl, move |_args| Err(tricky_message.to_string()))
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None);
    assert!(result.is_err());
    let msg = format!("{:#}", result.unwrap_err());
    assert!(
        msg.contains(tricky_message),
        "expected message to survive intact, got: {msg}"
    );
}

#[test]
fn test_extension_unknown_extension_is_evaluation_error_not_a_map() {
    // An unknown extension must make `Engine::eval` return `Err`. It must
    // not return an `{"error": ...}` map value.
    let decl = global_decl("abs", 1);
    let wasm = compile_with_extension(&decl, "abs(x)");
    // No runtime implementation registered at all.
    let result = runtime::Builder::new()
        .with_logger(create_test_logger())
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(Some(r#"{"x": -5}"#));
    let err = result.expect_err("expected a runtime error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("Extension not found"),
        "expected 'Extension not found', got: {msg}"
    );
    // A missing implementation is reported against the extension call site.
    let cel_err = err
        .downcast_ref::<CelRuntimeError>()
        .expect("error must downcast to CelRuntimeError");
    assert_eq!(
        cel_err.origin,
        Some(ExtensionOrigin {
            namespace: None,
            function: "abs".to_string(),
        })
    );
}

#[test]
fn test_extensions_and_build_pre_rehydrate() {
    // Test the `Extensions` and `EnginePre::rehydrate` path end to end.
    let decl = global_decl("abs", 1);
    let wasm = compile_with_extension(&decl, "abs(x)");

    let engine_pre = runtime::Builder::new()
        .with_wasm(wasm)
        .build_pre()
        .expect("build_pre failed");

    let mut extensions = Extensions::new();
    extensions.register(decl, |args| {
        let n = args[0].as_i64().unwrap_or(0);
        Ok(serde_json::Value::Number(n.abs().into()))
    });

    let result = engine_pre
        .rehydrate(extensions, create_test_logger(), None)
        .eval(Some(r#"{"x": -42}"#))
        .expect("eval failed");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 42);
}

// ============================================================
// Extension Authorizer Tests
// ============================================================

#[test]
fn test_extension_authorizer_denial_is_a_cel_runtime_error() {
    // An authorizer denial reaches the guest in the same way as an `Err`
    // from the closure: a `CelRuntimeError` with the extension as its
    // origin. The closure never runs.
    let decl = global_decl("myFunc", 1);
    let wasm = compile_with_extension(&decl, "myFunc(21)");
    let called = std::sync::Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();

    let result = runtime::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl, move |args| {
            called_clone.store(true, Ordering::SeqCst);
            let n = args[0].as_i64().unwrap_or(0);
            Ok(serde_json::Value::Number((n * 2).into()))
        })
        .with_extension_authorizer(|_key| Err("no access".to_string()))
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None);

    assert!(!called.load(Ordering::SeqCst), "closure must not run");
    let err = result.expect_err("expected a runtime error");
    let cel_err = err
        .downcast_ref::<CelRuntimeError>()
        .expect("error must downcast to CelRuntimeError");
    assert_eq!(cel_err.message, "no access");
    assert_eq!(
        cel_err.origin,
        Some(ExtensionOrigin {
            namespace: None,
            function: "myFunc".to_string(),
        })
    );
}

#[test]
fn test_extension_authorizer_denial_is_absorbed_by_or() {
    // `myFunc(21) == 42 || true` must evaluate to `true`. The denial behaves
    // like every other CEL runtime error.
    let decl = global_decl("myFunc", 1);
    let wasm = compile_with_extension(&decl, "myFunc(21) == 42 || true");

    let result = runtime::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl, |args| {
            let n = args[0].as_i64().unwrap_or(0);
            Ok(serde_json::Value::Number((n * 2).into()))
        })
        .with_extension_authorizer(|_key| Err("no access".to_string()))
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("error should be absorbed by ||");

    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_bool(), Some(true));
}

#[test]
fn test_extension_authorizer_allows_when_ok() {
    let decl = global_decl("myFunc", 1);
    let wasm = compile_with_extension(&decl, "myFunc(21)");
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let seen_clone = seen.clone();

    let result = runtime::Builder::new()
        .with_logger(create_test_logger())
        .with_extension(decl, |args| {
            let n = args[0].as_i64().unwrap_or(0);
            Ok(serde_json::Value::Number((n * 2).into()))
        })
        .with_extension_authorizer(move |key| {
            *seen_clone.lock().unwrap() = Some(key.clone());
            Ok(())
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");

    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64(), Some(42));
    assert_eq!(
        *seen.lock().unwrap(),
        Some(ferricel_core::ExtensionKey::new(None, "myFunc".to_string()))
    );
}

#[test]
fn test_extension_authorizer_via_extensions_and_rehydrate() {
    // The `EnginePre::rehydrate` path also uses an authorizer set
    // directly on `Extensions`.
    let decl = global_decl("myFunc", 1);
    let wasm = compile_with_extension(&decl, "myFunc(21)");
    let engine_pre = runtime::Builder::new()
        .with_wasm(wasm)
        .build_pre()
        .expect("build_pre failed");

    let extensions = Extensions::new()
        .with(decl, |args| {
            let n = args[0].as_i64().unwrap_or(0);
            Ok(serde_json::Value::Number((n * 2).into()))
        })
        .with_extension_authorizer(|_key| Err("no access".to_string()));

    let err = engine_pre
        .rehydrate(extensions, create_test_logger(), None)
        .eval(None)
        .expect_err("expected a runtime error");
    let cel_err = err
        .downcast_ref::<CelRuntimeError>()
        .expect("error must downcast to CelRuntimeError");
    assert_eq!(cel_err.message, "no access");
}
