// Tests for namespace / container-aware variable resolution.
//
// CEL spec: when a container is set (e.g. "com.example"), an unqualified
// identifier `y` should first resolve as `com.example.y`, then fall back to `y`.
// A dotted expression `x.y` should first try to resolve as a qualified variable
// named `"x.y"`, and only fall back to field access if that lookup misses.

mod common;
use common::*;

// ── Qualified variable lookup (x.y resolves to binding "x.y") ───────────────

#[test]
fn test_qualified_ident_resolves_to_bound_variable() {
    // `x.y` with binding "x.y" = true should resolve as a variable, not field access
    let result =
        compile_and_execute_with_container("x.y", None, serde_json::json!({ "x.y": true }))
            .expect("should succeed");
    assert_eq!(result, serde_json::Value::Bool(true));
}

#[test]
fn test_qualified_ident_falls_back_to_field_access() {
    // `x.y` where only "x" is bound (as a map with field "y") → field access
    let result =
        compile_and_execute_with_container("x.y", None, serde_json::json!({ "x": { "y": 42 } }))
            .expect("should succeed");
    assert_eq!(result, serde_json::json!(42));
}

#[test]
fn test_qualified_ident_prefers_variable_over_field_access() {
    // When BOTH "x.y" (=true) and "x" (={"y":false}) exist, qualified var wins
    let result = compile_and_execute_with_container(
        "x.y",
        None,
        serde_json::json!({ "x.y": true, "x": { "y": false } }),
    )
    .expect("should succeed");
    assert_eq!(result, serde_json::Value::Bool(true));
}

// ── Container-prefixed identifier resolution ─────────────────────────────────

#[test]
fn test_container_resolves_simple_ident() {
    // container "x", expr "y", binding "x.y" = true → resolves as x.y
    let result =
        compile_and_execute_with_container("y", Some("x"), serde_json::json!({ "x.y": true }))
            .expect("should succeed");
    assert_eq!(result, serde_json::Value::Bool(true));
}

#[test]
fn test_container_falls_back_to_root() {
    // container "x", expr "y", only "y" is bound (not "x.y") → falls back to root
    let result = compile_and_execute_with_container("y", Some("x"), serde_json::json!({ "y": 99 }))
        .expect("should succeed");
    assert_eq!(result, serde_json::json!(99));
}

#[test]
fn test_container_prefers_namespaced_over_root() {
    // container "x", both "x.y"=true and "y"=false bound → namespaced wins
    let result = compile_and_execute_with_container(
        "y",
        Some("x"),
        serde_json::json!({ "x.y": true, "y": false }),
    )
    .expect("should succeed");
    assert_eq!(result, serde_json::Value::Bool(true));
}

#[test]
fn test_multi_segment_container_resolution() {
    // container "com.example", expr "y", binding "com.example.y" = true
    let result = compile_and_execute_with_container(
        "y",
        Some("com.example"),
        serde_json::json!({ "com.example.y": true }),
    )
    .expect("should succeed");
    assert_eq!(result, serde_json::Value::Bool(true));
}

#[test]
fn test_multi_segment_container_partial_fallback() {
    // container "com.example", binding "com.y" (partial prefix) → should NOT match
    // should fall back to root "y"
    let result =
        compile_and_execute_with_container("y", Some("com.example"), serde_json::json!({ "y": 7 }))
            .expect("should succeed");
    assert_eq!(result, serde_json::json!(7));
}

// ── Container + qualified variable ───────────────────────────────────────────

#[test]
fn test_container_qualified_variable() {
    // container "x", expr "a.b", binding "x.a.b" = true
    let result =
        compile_and_execute_with_container("a.b", Some("x"), serde_json::json!({ "x.a.b": true }))
            .expect("should succeed");
    assert_eq!(result, serde_json::Value::Bool(true));
}

// ── Comprehension locals shadow container resolution ─────────────────────────

#[test]
fn test_comprehension_local_shadows_container_variable() {
    // container "com.example", binding "com.example.y" = 42
    // In [0].exists(y, y == 0), the iteration var `y` (=0) shadows "com.example.y"
    let result = compile_and_execute_with_container(
        "[0].exists(y, y == 0)",
        Some("com.example"),
        serde_json::json!({ "com.example.y": 42 }),
    )
    .expect("should succeed");
    assert_eq!(result, serde_json::Value::Bool(true));
}

#[test]
fn test_comprehension_local_field_access_not_affected() {
    // When `y` is a comprehension local, `y.z` must be field access on y, not a
    // qualified variable lookup for "y.z"
    let result = compile_and_execute_with_container(
        "[{'z': 0}].exists(y, y.z == 0)",
        None,
        serde_json::json!({ "y.z": 42 }),
    )
    .expect("should succeed");
    // y iterates over [{"z":0}], y.z == 0 → true
    assert_eq!(result, serde_json::Value::Bool(true));
}
