//! Compiler support for fluent builder chain extensions.
//!
//! Handles [`BuilderStep::Entry`], [`BuilderStep::Chain`], and
//! [`BuilderStep::Terminal`] steps declared via
//! [`BuilderChainDecl`](ferricel_types::extensions::BuilderChainDecl).
//!
//! ## Runtime representation
//!
//! Each intermediate builder object is a `CelValue::Object` (map) with a
//! reserved `"__type__"` key.  Example after
//! `kw.k8s.apiVersion("v1").kind("Pod").namespace("default")`:
//!
//! ```json
//! { "__type__": "kw.k8s.Client",
//!   "apiVersion": "v1",
//!   "kind": "Pod",
//!   "namespace": "default" }
//! ```
//!
//! The host receives this map as the first argument of the terminal
//! [`ExtensionCallPayload`](ferricel_types::extensions::ExtensionCallPayload).

use cel::common::ast::CallExpr;
use ferricel_types::{extensions::BuilderStep, functions::RuntimeFunction};
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::{emit_string_const, get_memory_id},
};

// ─── Entry step ───────────────────────────────────────────────────────────────

/// Compile a builder **Entry** step.
///
/// Entry steps are called as a plain global function with a dotted name, e.g.
/// `kw.k8s.apiVersion("v1")`.  The CEL parser sees `kw` as the target ident
/// and `k8s.apiVersion` as chained selects — but by the time we reach this
/// function the compiler has already matched the full dotted name in the
/// registry.
///
/// The `call_expr` passed in has:
/// - `func_name` = the last segment (e.g. `"apiVersion"`)
/// - `target`    = the preceding dotted-name ident chain (not compiled)
/// - `args`      = `[arg0]` — the single value for `state_key`
///
/// We emit:
/// ```text
/// i32.const 0          ; null receiver (fresh map)
/// <type_tag ptr+len>
/// <key ptr+len>
/// <compile arg0>       ; the value
/// i32.const 0          ; accumulate = false
/// call cel_builder_step
/// ```
pub fn compile_builder_entry(
    step: &BuilderStep,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let (state_key, output_type) = match step {
        BuilderStep::Entry {
            state_key,
            output_type,
            ..
        } => (state_key.as_str(), output_type.as_str()),
        _ => anyhow::bail!("compile_builder_entry called with non-Entry step"),
    };

    // Validate arity: exactly one argument.
    if call_expr.args.len() != 1 {
        anyhow::bail!(
            "Builder entry '{}' expects 1 argument, got {}",
            call_expr.func_name,
            call_expr.args.len()
        );
    }

    let mem = get_memory_id(module)?;

    // null receiver → fresh map
    body.i32_const(0);

    // type tag
    emit_string_const(output_type, body, env, mem, module);

    // state key
    emit_string_const(state_key, body, env, mem, module);

    // argument value
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;

    // accumulate = false
    body.i32_const(0);

    body.call(env.get(RuntimeFunction::BuilderStepCall));
    Ok(())
}

// ─── Chain step ───────────────────────────────────────────────────────────────

/// Compile a builder **Chain** step.
///
/// Chain steps are receiver-style calls, e.g. `<builder>.kind("Pod")`.
///
/// The `call_expr` has:
/// - `target` = `Some(receiver_expr)` — the builder object to update
/// - `args`   = `[arg0]` — the new value (or two args for 3-arg methods if needed)
///
/// We emit:
/// ```text
/// <compile receiver>   ; existing state map
/// <type_tag ptr+len>
/// <key ptr+len>
/// <compile arg0>       ; the value
/// i32.const accumulate ; 0 or 1
/// call cel_builder_step
/// ```
pub fn compile_builder_chain(
    step: &BuilderStep,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let (state_key, output_type, accumulate) = match step {
        BuilderStep::Chain {
            state_key,
            output_type,
            accumulate,
            ..
        } => (state_key.as_str(), output_type.as_str(), *accumulate),
        _ => anyhow::bail!("compile_builder_chain called with non-Chain step"),
    };

    let receiver = call_expr.target.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "Builder chain step '{}' requires a receiver",
            call_expr.func_name
        )
    })?;

    if call_expr.args.len() != 1 {
        anyhow::bail!(
            "Builder chain step '{}' expects 1 argument, got {}",
            call_expr.func_name,
            call_expr.args.len()
        );
    }

    let mem = get_memory_id(module)?;

    // receiver (existing map)
    compile_expr(&receiver.expr, body, env, ctx, module)?;

    // type tag
    emit_string_const(output_type, body, env, mem, module);

    // state key
    emit_string_const(state_key, body, env, mem, module);

    // argument value
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;

    // accumulate flag
    body.i32_const(if accumulate { 1 } else { 0 });

    body.call(env.get(RuntimeFunction::BuilderStepCall));
    Ok(())
}

// ─── Terminal step ────────────────────────────────────────────────────────────

/// Compile a builder **Terminal** step.
///
/// Terminal steps emit a host extension call ([`RuntimeFunction::ExtCall1`] or [`RuntimeFunction::ExtCall2`])
/// with the accumulated state map (and optional extra argument folded in).
///
/// For `.list()` (no extra arg):
/// ```text
/// <compile receiver>
/// call ExtCall1  ; host_namespace="kw.k8s", host_function="list"
/// ```
///
/// For `.get("nginx")` (`extra_arg_key = Some("name")`):
/// ```text
/// <compile receiver>            ; existing map
/// <type_tag ptr+len>            ; same output_type as the input_type
/// <"name" key ptr+len>
/// <compile arg0>                ; "nginx"
/// i32.const 0                   ; accumulate = false
/// call cel_builder_step         ; fold extra arg in → updated map
/// call ExtCall1                 ; host_namespace="kw.k8s", host_function="get"
/// ```
pub fn compile_builder_terminal(
    step: &BuilderStep,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let (extra_arg_key, host_namespace, host_function, input_type) = match step {
        BuilderStep::Terminal {
            extra_arg_key,
            host_namespace,
            host_function,
            input_type,
            ..
        } => (
            extra_arg_key.as_deref(),
            host_namespace.as_str(),
            host_function.as_str(),
            input_type.as_str(),
        ),
        _ => anyhow::bail!("compile_builder_terminal called with non-Terminal step"),
    };

    let receiver = call_expr.target.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "Builder terminal '{}' requires a receiver",
            call_expr.func_name
        )
    })?;

    let expected_extra_args = if extra_arg_key.is_some() { 1 } else { 0 };
    if call_expr.args.len() != expected_extra_args {
        anyhow::bail!(
            "Builder terminal '{}' expects {} argument(s), got {}",
            call_expr.func_name,
            expected_extra_args,
            call_expr.args.len()
        );
    }

    let mem = get_memory_id(module)?;

    // Compile the receiver (base state map).
    compile_expr(&receiver.expr, body, env, ctx, module)?;

    // If there's an extra argument, fold it into the map via cel_builder_step.
    if let Some(key) = extra_arg_key {
        emit_string_const(input_type, body, env, mem, module); // keep same __type__
        emit_string_const(key, body, env, mem, module);
        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
        body.i32_const(0); // accumulate = false
        body.call(env.get(RuntimeFunction::BuilderStepCall));
    }

    // Now emit the host ExtCall1: (ns_ptr, ns_len, fn_ptr, fn_len, map_ptr) → *CelValue
    // The map is already on the stack as the sole CelValue* argument.
    // We need to push (ns_ptr, ns_len, fn_ptr, fn_len) *before* the map arg,
    // but ExtCall1 expects: ns_ptr, ns_len, fn_ptr, fn_len, arg0_ptr.
    // We need a local to hold the map so we can push ns/fn first.
    let map_local = module.locals.add(walrus::ValType::I32);
    body.local_set(map_local);

    emit_string_const(host_namespace, body, env, mem, module);
    emit_string_const(host_function, body, env, mem, module);
    body.local_get(map_local);

    body.call(env.get(RuntimeFunction::ExtCall1));
    Ok(())
}

// ─── Dispatch helper ──────────────────────────────────────────────────────────

/// Find the best matching builder step for a given call and dispatch to the
/// appropriate compile function.
///
/// For **Entry** steps the lookup key is the full dotted function name
/// (e.g. `"kw.k8s.apiVersion"`).  For **Chain / Terminal** steps the lookup
/// key is the short method name (e.g. `"kind"`), and we pick the first
/// registered step that matches — since step function names within a chain
/// family are unique by design.
///
/// Returns `Ok(true)` if the call was handled, `Ok(false)` if no builder step
/// matched (so the caller can fall through to the flat extension handler).
pub fn try_compile_builder_call(
    full_name: &str, // dotted name used for Entry lookup
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<bool, anyhow::Error> {
    // 1. Check Entry steps (keyed by full dotted name).
    if let Some(step) = ctx.extensions.builder_entries.get(full_name) {
        compile_builder_entry(step, call_expr, body, env, ctx, module)?;
        return Ok(true);
    }

    // 2. Check Chain/Terminal steps (keyed by short method name).
    //    Only apply when the call has a receiver (target).
    if call_expr.target.is_some() {
        let short = &call_expr.func_name;
        if let Some(steps) = ctx.extensions.builder_steps.get(short.as_str()) {
            // Pick first matching step.  In practice there will often be only one,
            // but multiple chains can register the same method name (e.g. both
            // kw.sigstore and kw.crypto define `verify`).  The caller's
            // runtime type tag disambiguates at runtime.
            let step = &steps[0];
            match step {
                BuilderStep::Chain { .. } => {
                    compile_builder_chain(step, call_expr, body, env, ctx, module)?;
                }
                BuilderStep::Terminal { .. } => {
                    compile_builder_terminal(step, call_expr, body, env, ctx, module)?;
                }
                BuilderStep::Entry { .. } => unreachable!("Entry steps are not in builder_steps"),
            }
            return Ok(true);
        }
    }

    Ok(false)
}
