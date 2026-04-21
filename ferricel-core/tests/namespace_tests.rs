// Tests for namespace / container-aware variable resolution.
//
// CEL spec: when a container is set (e.g. "com.example"), an unqualified
// identifier `y` should first resolve as `com.example.y`, then fall back to `y`.
// A dotted expression `x.y` should first try to resolve as a qualified variable
// named `"x.y"`, and only fall back to field access if that lookup misses.

mod common;
use common::*;
use ferricel_core::runtime::CelEngine;
use ferricel_types::LogLevel;
use rstest::rstest;
use serde_json::json;

#[rstest]
// ── Qualified variable lookup (x.y resolves to binding "x.y") ───────────────
// `x.y` with binding "x.y" = true should resolve as a variable, not field access
#[case("x.y", None, json!({"x.y": true}), json!(true))]
// `x.y` where only "x" is bound (as a map with field "y") → field access
#[case("x.y", None, json!({"x": {"y": 42}}), json!(42))]
// When BOTH "x.y" (=true) and "x" (={"y":false}) exist, qualified var wins
#[case("x.y", None, json!({"x.y": true, "x": {"y": false}}), json!(true))]
// ── Container-prefixed identifier resolution ─────────────────────────────────
// container "x", expr "y", binding "x.y" = true → resolves as x.y
#[case("y", Some("x"), json!({"x.y": true}), json!(true))]
// container "x", expr "y", only "y" is bound (not "x.y") → falls back to root
#[case("y", Some("x"), json!({"y": 99}), json!(99))]
// container "x", both "x.y"=true and "y"=false bound → namespaced wins
#[case("y", Some("x"), json!({"x.y": true, "y": false}), json!(true))]
// container "com.example", expr "y", binding "com.example.y" = true
#[case("y", Some("com.example"), json!({"com.example.y": true}), json!(true))]
// container "com.example", binding "com.y" (partial prefix) → should NOT match, falls back to root "y"
#[case("y", Some("com.example"), json!({"y": 7}), json!(7))]
// ── Container + qualified variable ───────────────────────────────────────────
// container "x", expr "a.b", binding "x.a.b" = true
#[case("a.b", Some("x"), json!({"x.a.b": true}), json!(true))]
// ── Comprehension locals shadow container resolution ─────────────────────────
// In [0].exists(y, y == 0), the iteration var `y` (=0) shadows "com.example.y" (=42)
#[case("[0].exists(y, y == 0)", Some("com.example"), json!({"com.example.y": 42}), json!(true))]
// When `y` is a comprehension local, `y.z` must be field access on y, not a
// qualified variable lookup for "y.z" (binding "y.z"=42 must be ignored)
#[case("[{'z': 0}].exists(y, y.z == 0)", None, json!({"y.z": 42}), json!(true))]
fn test_container_resolution(
    #[case] expr: &str,
    #[case] container: Option<&str>,
    #[case] bindings: serde_json::Value,
    #[case] expected: serde_json::Value,
) {
    let wasm = compile_with_container(expr, container, None).unwrap();
    let bindings_str = serde_json::to_string(&bindings).unwrap();
    let result: serde_json::Value = serde_json::from_str(
        &CelEngine::new(create_test_logger())
            .with_log_level(LogLevel::Info)
            .execute(&wasm, Some(&bindings_str))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result, expected);
}
