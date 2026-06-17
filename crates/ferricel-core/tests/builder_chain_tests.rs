// Integration tests for builder chain extensions.

use ferricel_core::{compiler, runtime};
use ferricel_types::extensions::{BuilderChainDecl, BuilderStep, ExtensionDecl};

use crate::common::*;

/// A simple chain: `test.entry("v") → Chain.method("x") → Terminal.run()`
/// The terminal calls host extension `test/run` with the accumulated map.
fn simple_chain() -> BuilderChainDecl {
    BuilderChainDecl {
        steps: vec![
            BuilderStep::Entry {
                function: "test.entry".to_string(),
                state_keys: vec!["val".to_string()],
                output_type: "test.Builder".to_string(),
            },
            BuilderStep::Chain {
                function: "method".to_string(),
                input_type: "test.Builder".to_string(),
                state_keys: vec!["arg".to_string()],
                output_type: "test.Builder".to_string(),
                accumulate: false,
            },
            BuilderStep::Terminal {
                function: "run".to_string(),
                input_type: "test.Builder".to_string(),
                extra_arg_keys: vec![],
                host_namespace: "test".to_string(),
                host_function: "run".to_string(),
            },
        ],
    }
}

fn simple_chain_ext_decl() -> ExtensionDecl {
    ExtensionDecl {
        namespace: Some("test".to_string()),
        function: "run".to_string(),
        global_style: false,
        receiver_style: false,
        num_args: 1,
    }
}

/// Chain with a 2-arg chain step: `.twoArg("a", "b")` stores both keys.
fn multi_arg_chain() -> BuilderChainDecl {
    BuilderChainDecl {
        steps: vec![
            BuilderStep::Entry {
                function: "multi.start".to_string(),
                state_keys: vec!["image".to_string()],
                output_type: "multi.Builder".to_string(),
            },
            BuilderStep::Chain {
                function: "twoArg".to_string(),
                input_type: "multi.Builder".to_string(),
                state_keys: vec!["issuer".to_string(), "subject".to_string()],
                output_type: "multi.Ready".to_string(),
                accumulate: false,
            },
            BuilderStep::Terminal {
                function: "execute".to_string(),
                input_type: "multi.Ready".to_string(),
                extra_arg_keys: vec![],
                host_namespace: "multi".to_string(),
                host_function: "execute".to_string(),
            },
        ],
    }
}

fn multi_ext_decl() -> ExtensionDecl {
    ExtensionDecl {
        namespace: Some("multi".to_string()),
        function: "execute".to_string(),
        global_style: false,
        receiver_style: false,
        num_args: 1,
    }
}

#[test]
fn test_builder_multi_arg_chain_step() {
    let logger = create_test_logger();
    let wasm = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_builder_chain(multi_arg_chain())
        .build()
        .compile("multi.start('img').twoArg('iss', 'sub').execute()")
        .expect("compile failed");

    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(multi_ext_decl(), |args| {
            // The host receives the accumulated map.
            let map = &args[0];
            assert_eq!(map["image"], "img");
            assert_eq!(map["issuer"], "iss");
            assert_eq!(map["subject"], "sub");
            Ok(serde_json::json!(true))
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");

    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value, true);
}

/// Terminal with extra args: `.finish("name", "ns")` folds both before calling host.
#[test]
fn test_builder_multi_arg_terminal() {
    let chain = BuilderChainDecl {
        steps: vec![
            BuilderStep::Entry {
                function: "term.start".to_string(),
                state_keys: vec!["kind".to_string()],
                output_type: "term.Builder".to_string(),
            },
            BuilderStep::Terminal {
                function: "finish".to_string(),
                input_type: "term.Builder".to_string(),
                extra_arg_keys: vec!["name".to_string(), "namespace".to_string()],
                host_namespace: "term".to_string(),
                host_function: "finish".to_string(),
            },
        ],
    };
    let ext_decl = ExtensionDecl {
        namespace: Some("term".to_string()),
        function: "finish".to_string(),
        global_style: false,
        receiver_style: false,
        num_args: 1,
    };

    let logger = create_test_logger();
    let wasm = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_builder_chain(chain)
        .build()
        .compile("term.start('Pod').finish('nginx', 'default')")
        .expect("compile failed");

    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(ext_decl, |args| {
            let map = &args[0];
            assert_eq!(map["kind"], "Pod");
            assert_eq!(map["name"], "nginx");
            assert_eq!(map["namespace"], "default");
            Ok(serde_json::json!("ok"))
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");

    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value, "ok");
}

/// Two chains share the method name `verify` but with different input_types
/// and host_functions. The compiler must route each to the correct host call.
#[test]
fn test_builder_type_disambiguation() {
    // Chain A: chainA.start("x").verify() → host_function "verifyA"
    let chain_a = BuilderChainDecl {
        steps: vec![
            BuilderStep::Entry {
                function: "chainA.start".to_string(),
                state_keys: vec!["val".to_string()],
                output_type: "chainA.Builder".to_string(),
            },
            BuilderStep::Terminal {
                function: "verify".to_string(),
                input_type: "chainA.Builder".to_string(),
                extra_arg_keys: vec![],
                host_namespace: "chainA".to_string(),
                host_function: "verifyA".to_string(),
            },
        ],
    };
    // Chain B: chainB.start("y").verify() → host_function "verifyB"
    let chain_b = BuilderChainDecl {
        steps: vec![
            BuilderStep::Entry {
                function: "chainB.start".to_string(),
                state_keys: vec!["val".to_string()],
                output_type: "chainB.Builder".to_string(),
            },
            BuilderStep::Terminal {
                function: "verify".to_string(),
                input_type: "chainB.Builder".to_string(),
                extra_arg_keys: vec![],
                host_namespace: "chainB".to_string(),
                host_function: "verifyB".to_string(),
            },
        ],
    };

    let ext_a = ExtensionDecl {
        namespace: Some("chainA".to_string()),
        function: "verifyA".to_string(),
        global_style: false,
        receiver_style: false,
        num_args: 1,
    };
    let ext_b = ExtensionDecl {
        namespace: Some("chainB".to_string()),
        function: "verifyB".to_string(),
        global_style: false,
        receiver_style: false,
        num_args: 1,
    };

    let logger = create_test_logger();

    // Compile and run an expression using chain A's verify.
    let wasm_a = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_builder_chain(chain_a.clone())
        .with_builder_chain(chain_b.clone())
        .build()
        .compile("chainA.start('hello').verify()")
        .expect("compile A failed");

    let result_a = runtime::Builder::new()
        .with_logger(logger.clone())
        .with_extension(ext_a.clone(), |args| {
            assert_eq!(args[0]["val"], "hello");
            Ok(serde_json::json!("A"))
        })
        .with_extension(ext_b.clone(), |_| Ok(serde_json::json!("B")))
        .with_wasm(wasm_a)
        .build()
        .expect("build A failed")
        .eval(None)
        .expect("eval A failed");
    let val_a: serde_json::Value = serde_json::from_str(&result_a).unwrap();
    assert_eq!(val_a, "A", "chainA.start should route to verifyA");

    // Compile and run an expression using chain B's verify.
    let wasm_b = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_builder_chain(chain_a)
        .with_builder_chain(chain_b)
        .build()
        .compile("chainB.start('world').verify()")
        .expect("compile B failed");

    let result_b = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(ext_a, |_| Ok(serde_json::json!("A")))
        .with_extension(ext_b, |args| {
            assert_eq!(args[0]["val"], "world");
            Ok(serde_json::json!("B"))
        })
        .with_wasm(wasm_b)
        .build()
        .expect("build B failed")
        .eval(None)
        .expect("eval B failed");
    let val_b: serde_json::Value = serde_json::from_str(&result_b).unwrap();
    assert_eq!(val_b, "B", "chainB.start should route to verifyB");
}

/// Within-chain disambiguation: `.pubKey()` appears as both a type-transition
/// step (Builder → Verifier) and an accumulation step (Verifier → Verifier),
/// distinguished by input_type.
#[test]
fn test_builder_within_chain_type_transition() {
    let chain = BuilderChainDecl {
        steps: vec![
            BuilderStep::Entry {
                function: "sig.image".to_string(),
                state_keys: vec!["image".to_string()],
                output_type: "sig.VerifierBuilder".to_string(),
            },
            // First pubKey: transition from VerifierBuilder → PubKeysVerifier
            BuilderStep::Chain {
                function: "pubKey".to_string(),
                input_type: "sig.VerifierBuilder".to_string(),
                state_keys: vec!["pubKeys".to_string()],
                output_type: "sig.PubKeysVerifier".to_string(),
                accumulate: false,
            },
            // Second pubKey: accumulate on PubKeysVerifier → PubKeysVerifier
            BuilderStep::Chain {
                function: "pubKey".to_string(),
                input_type: "sig.PubKeysVerifier".to_string(),
                state_keys: vec!["pubKeys".to_string()],
                output_type: "sig.PubKeysVerifier".to_string(),
                accumulate: true,
            },
            BuilderStep::Terminal {
                function: "verify".to_string(),
                input_type: "sig.PubKeysVerifier".to_string(),
                extra_arg_keys: vec![],
                host_namespace: "sig".to_string(),
                host_function: "pubKeyVerify".to_string(),
            },
        ],
    };
    let ext_decl = ExtensionDecl {
        namespace: Some("sig".to_string()),
        function: "pubKeyVerify".to_string(),
        global_style: false,
        receiver_style: false,
        num_args: 1,
    };

    let logger = create_test_logger();
    // sig.image('img').pubKey('key1').pubKey('key2').verify()
    // First .pubKey('key1') transitions: output_type = PubKeysVerifier, accumulate=false
    // Second .pubKey('key2') accumulates: output_type = PubKeysVerifier, accumulate=true
    // So pubKeys should be an array ["key1", "key2"]
    let wasm = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_builder_chain(chain)
        .build()
        .compile("sig.image('img').pubKey('key1').pubKey('key2').verify()")
        .expect("compile failed");

    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(ext_decl, |args| {
            let map = &args[0];
            assert_eq!(map["image"], "img");
            // First pubKey overwrites (accumulate=false), second appends (accumulate=true)
            // → ["key1", "key2"]
            assert_eq!(map["pubKeys"], serde_json::json!(["key1", "key2"]));
            Ok(serde_json::json!("verified"))
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");

    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value, "verified");
}

/// Same method name `action`, two arities: 1-arg and 2-arg.
#[test]
fn test_builder_arity_overload() {
    let chain = BuilderChainDecl {
        steps: vec![
            BuilderStep::Entry {
                function: "ov.start".to_string(),
                state_keys: vec!["image".to_string()],
                output_type: "ov.Builder".to_string(),
            },
            // 1-arg: .action("owner") → sets "owner" only
            BuilderStep::Chain {
                function: "action".to_string(),
                input_type: "ov.Builder".to_string(),
                state_keys: vec!["owner".to_string()],
                output_type: "ov.Ready".to_string(),
                accumulate: false,
            },
            // 2-arg: .action("owner", "repo") → sets both
            BuilderStep::Chain {
                function: "action".to_string(),
                input_type: "ov.Builder".to_string(),
                state_keys: vec!["owner".to_string(), "repo".to_string()],
                output_type: "ov.Ready".to_string(),
                accumulate: false,
            },
            BuilderStep::Terminal {
                function: "go".to_string(),
                input_type: "ov.Ready".to_string(),
                extra_arg_keys: vec![],
                host_namespace: "ov".to_string(),
                host_function: "go".to_string(),
            },
        ],
    };
    let ext_decl = ExtensionDecl {
        namespace: Some("ov".to_string()),
        function: "go".to_string(),
        global_style: false,
        receiver_style: false,
        num_args: 1,
    };

    let logger = create_test_logger();

    // 1-arg form
    let wasm_1 = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_builder_chain(chain.clone())
        .build()
        .compile("ov.start('img').action('myorg').go()")
        .expect("compile 1-arg failed");

    let result_1 = runtime::Builder::new()
        .with_logger(logger.clone())
        .with_extension(ext_decl.clone(), |args| {
            let map = &args[0];
            assert_eq!(map["owner"], "myorg");
            assert!(map.get("repo").is_none(), "repo should not be set");
            Ok(serde_json::json!("one"))
        })
        .with_wasm(wasm_1)
        .build()
        .expect("build 1-arg failed")
        .eval(None)
        .expect("eval 1-arg failed");
    let val_1: serde_json::Value = serde_json::from_str(&result_1).unwrap();
    assert_eq!(val_1, "one");

    // 2-arg form
    let wasm_2 = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_builder_chain(chain)
        .build()
        .compile("ov.start('img').action('myorg', 'myrepo').go()")
        .expect("compile 2-arg failed");

    let result_2 = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(ext_decl, |args| {
            let map = &args[0];
            assert_eq!(map["owner"], "myorg");
            assert_eq!(map["repo"], "myrepo");
            Ok(serde_json::json!("two"))
        })
        .with_wasm(wasm_2)
        .build()
        .expect("build 2-arg failed")
        .eval(None)
        .expect("eval 2-arg failed");
    let val_2: serde_json::Value = serde_json::from_str(&result_2).unwrap();
    assert_eq!(val_2, "two");
}

// ============================================================
// MapEntry: dynamic-key accumulation
// ============================================================

#[test]
fn test_builder_map_entry_single() {
    let chain = BuilderChainDecl {
        steps: vec![
            BuilderStep::Entry {
                function: "me.start".to_string(),
                state_keys: vec!["image".to_string()],
                output_type: "me.Builder".to_string(),
            },
            BuilderStep::MapEntry {
                function: "annotation".to_string(),
                input_type: "me.Builder".to_string(),
                state_key: "annotations".to_string(),
                output_type: "me.Builder".to_string(),
            },
            BuilderStep::Terminal {
                function: "run".to_string(),
                input_type: "me.Builder".to_string(),
                extra_arg_keys: vec![],
                host_namespace: "me".to_string(),
                host_function: "run".to_string(),
            },
        ],
    };
    let ext_decl = ExtensionDecl {
        namespace: Some("me".to_string()),
        function: "run".to_string(),
        global_style: false,
        receiver_style: false,
        num_args: 1,
    };

    let logger = create_test_logger();
    let wasm = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_builder_chain(chain)
        .build()
        .compile("me.start('img').annotation('env', 'prod').run()")
        .expect("compile failed");

    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(ext_decl, |args| {
            let map = &args[0];
            assert_eq!(map["image"], "img");
            assert_eq!(map["annotations"]["env"], "prod");
            Ok(serde_json::json!("ok"))
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");

    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value, "ok");
}

#[test]
fn test_builder_map_entry_accumulates() {
    let chain = BuilderChainDecl {
        steps: vec![
            BuilderStep::Entry {
                function: "me.start".to_string(),
                state_keys: vec!["image".to_string()],
                output_type: "me.Builder".to_string(),
            },
            BuilderStep::MapEntry {
                function: "annotation".to_string(),
                input_type: "me.Builder".to_string(),
                state_key: "annotations".to_string(),
                output_type: "me.Builder".to_string(),
            },
            BuilderStep::Terminal {
                function: "run".to_string(),
                input_type: "me.Builder".to_string(),
                extra_arg_keys: vec![],
                host_namespace: "me".to_string(),
                host_function: "run".to_string(),
            },
        ],
    };
    let ext_decl = ExtensionDecl {
        namespace: Some("me".to_string()),
        function: "run".to_string(),
        global_style: false,
        receiver_style: false,
        num_args: 1,
    };

    let logger = create_test_logger();
    // Two .annotation() calls should merge into the same nested map.
    let wasm = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_builder_chain(chain)
        .build()
        .compile("me.start('img').annotation('env', 'prod').annotation('team', 'sec').run()")
        .expect("compile failed");

    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(ext_decl, |args| {
            let map = &args[0];
            assert_eq!(map["annotations"]["env"], "prod");
            assert_eq!(map["annotations"]["team"], "sec");
            Ok(serde_json::json!("merged"))
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");

    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value, "merged");
}

// ============================================================
// Basic regression: simple chain still works
// ============================================================

#[test]
fn test_builder_simple_chain() {
    let logger = create_test_logger();
    let wasm = compiler::Builder::new()
        .with_logger(create_test_logger())
        .with_builder_chain(simple_chain())
        .build()
        .compile("test.entry('hello').method('world').run()")
        .expect("compile failed");

    let result = runtime::Builder::new()
        .with_logger(logger)
        .with_extension(simple_chain_ext_decl(), |args| {
            let map = &args[0];
            assert_eq!(map["val"], "hello");
            assert_eq!(map["arg"], "world");
            Ok(serde_json::json!(42))
        })
        .with_wasm(wasm)
        .build()
        .expect("build failed")
        .eval(None)
        .expect("eval failed");

    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value.as_i64().unwrap(), 42);
}
