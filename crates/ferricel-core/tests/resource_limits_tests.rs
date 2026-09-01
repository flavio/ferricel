// Integration tests for `runtime::Builder::with_resource_limits`, which
// leverages wasmtime's `ResourceLimiter` facility (via `StoreLimits`) to cap
// the linear memory a single evaluation is allowed to allocate.
//
// These tests intentionally do not use the shared `compile_and_execute*`
// helpers in `common.rs` because those never configure resource limits.

use ferricel_core::{compiler, runtime, runtime::ResourceLimits};

// Per the WebAssembly specification, linear memory is grown in units of
// pages, and a page is fixed at 64KiB.
const WASM_PAGE_SIZE: usize = 65536;

/// Compile `cel_expr` and evaluate it against `bindings_json`, optionally
/// enforcing `resource_limits`.
fn eval_with_resource_limits(
    cel_expr: &str,
    bindings_json: Option<&str>,
    resource_limits: Option<ResourceLimits>,
) -> Result<String, anyhow::Error> {
    let wasm = compiler::Builder::new().build().compile(cel_expr)?;

    let mut builder = runtime::Builder::new().with_wasm(wasm);
    if let Some(limits) = resource_limits {
        builder = builder.with_resource_limits(limits);
    }

    builder.build()?.eval(bindings_json)
}

/// Builds a CEL expression that computes the length of a large string built
/// entirely inside the Wasm module (no large host-provided bindings needed):
/// `size(lists.range(n).map(i, string(i)).join(","))`.
fn big_string_expr(n: u64) -> String {
    format!(r#"size(lists.range({n}).map(i, string(i)).join(","))"#)
}

#[test]
fn no_limits_allows_large_allocation() {
    // Without any resource limit configured, computing a fairly large string
    // (a few hundred KB) succeeds.
    let expr = big_string_expr(50_000);

    let result = eval_with_resource_limits(&expr, None, None);

    assert!(
        result.is_ok(),
        "expected evaluation to succeed without resource limits, got: {:?}",
        result
    );
}

#[test]
fn generous_limit_allows_large_allocation() {
    // A generous limit (64 MiB) is well above what's needed, so evaluation
    // must succeed.
    let expr = big_string_expr(50_000);
    let resource_limits = ResourceLimits {
        max_memory_size: Some(64 * 1024 * 1024),
        ..Default::default()
    };

    let result = eval_with_resource_limits(&expr, None, Some(resource_limits));

    assert!(
        result.is_ok(),
        "expected evaluation to succeed with a generous resource limit, got: {:?}",
        result
    );
}

#[test]
fn tight_limit_blocks_large_allocation() {
    // The module's own initial linear memory already occupies a few pages
    // (well under 1 MiB), so a 3 MiB cap comfortably allows instantiation but
    // must block the growth needed to compute a much larger string.
    let expr = big_string_expr(500_000);
    let resource_limits = ResourceLimits {
        max_memory_size: Some(3 * 1024 * 1024),
        ..Default::default()
    };

    let result = eval_with_resource_limits(&expr, None, Some(resource_limits));

    assert!(
        result.is_err(),
        "expected evaluation to fail once the memory limit is exceeded, got: {:?}",
        result
    );
}

#[test]
fn limit_below_initial_memory_fails_at_instantiation() {
    // A cap smaller than half a Wasm page is below the module's own initial
    // linear memory, so even instantiating the module must fail.
    let resource_limits = ResourceLimits {
        max_memory_size: Some(WASM_PAGE_SIZE / 2),
        ..Default::default()
    };

    let result = eval_with_resource_limits("1 + 1", None, Some(resource_limits));

    assert!(
        result.is_err(),
        "expected instantiation to fail because initial memory exceeds the limit, got: {:?}",
        result
    );
}

#[test]
fn default_resource_limits_is_a_noop() {
    // `ResourceLimits::default()` (all fields `None`) must behave exactly
    // like passing no limits at all.
    let expr = big_string_expr(50_000);

    let result = eval_with_resource_limits(&expr, None, Some(ResourceLimits::default()));

    assert!(
        result.is_ok(),
        "expected evaluation to succeed with default (unlimited) resource limits, got: {:?}",
        result
    );
}
