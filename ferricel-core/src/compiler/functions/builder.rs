//! Compiler support for fluent builder chain extensions.
//!
//! Handles [`BuilderStep::Entry`], [`BuilderStep::Chain`],
//! [`BuilderStep::MapEntry`], and [`BuilderStep::Terminal`] steps declared via
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

use cel::common::ast::{CallExpr, Expr};
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
/// For each positional argument `i` we emit one `cel_builder_step` call,
/// threading the resulting map pointer into the next call.  The first call
/// starts from a null receiver (fresh map).
pub fn compile_builder_entry(
    step: &BuilderStep,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let (state_keys, output_type) = match step {
        BuilderStep::Entry {
            state_keys,
            output_type,
            ..
        } => (state_keys, output_type.as_str()),
        _ => anyhow::bail!("compile_builder_entry called with non-Entry step"),
    };

    if call_expr.args.len() != state_keys.len() {
        anyhow::bail!(
            "Builder entry '{}' expects {} argument(s), got {}",
            call_expr.func_name,
            state_keys.len(),
            call_expr.args.len()
        );
    }

    let mem = get_memory_id(module)?;

    for (i, key) in state_keys.iter().enumerate() {
        // First iteration: null receiver (fresh map); subsequent: previous result on stack.
        if i == 0 {
            body.i32_const(0);
        }

        emit_string_const(output_type, body, env, mem, module);
        emit_string_const(key, body, env, mem, module);
        compile_expr(&call_expr.args[i].expr, body, env, ctx, module)?;
        body.i32_const(0); // accumulate = false
        body.call(env.get(RuntimeFunction::BuilderStepCall));
    }

    Ok(())
}

// ─── Chain step ───────────────────────────────────────────────────────────────

/// Compile a builder **Chain** step.
///
/// Chain steps are receiver-style calls, e.g. `<builder>.kind("Pod")`.
/// For multi-arg steps (e.g. `.keyless("issuer", "subject")`), we emit one
/// `cel_builder_step` per positional argument, threading the map through.
pub fn compile_builder_chain(
    step: &BuilderStep,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let (state_keys, output_type, accumulate) = match step {
        BuilderStep::Chain {
            state_keys,
            output_type,
            accumulate,
            ..
        } => (state_keys, output_type.as_str(), *accumulate),
        _ => anyhow::bail!("compile_builder_chain called with non-Chain step"),
    };

    let receiver = call_expr.target.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "Builder chain step '{}' requires a receiver",
            call_expr.func_name
        )
    })?;

    if call_expr.args.len() != state_keys.len() {
        anyhow::bail!(
            "Builder chain step '{}' expects {} argument(s), got {}",
            call_expr.func_name,
            state_keys.len(),
            call_expr.args.len()
        );
    }

    let mem = get_memory_id(module)?;

    // Compile the receiver (existing map) — this is the initial value on the stack.
    compile_expr(&receiver.expr, body, env, ctx, module)?;

    for (i, key) in state_keys.iter().enumerate() {
        emit_string_const(output_type, body, env, mem, module);
        emit_string_const(key, body, env, mem, module);
        compile_expr(&call_expr.args[i].expr, body, env, ctx, module)?;
        body.i32_const(if accumulate { 1 } else { 0 });
        body.call(env.get(RuntimeFunction::BuilderStepCall));
    }

    Ok(())
}

// ─── MapEntry step ────────────────────────────────────────────────────────────

/// Compile a builder **MapEntry** step.
///
/// MapEntry steps insert a runtime key→value pair into a nested map within
/// the builder state.  Always takes exactly 2 arguments.
///
/// Example: `.annotation("env", "prod")` emits:
/// ```text
/// <compile receiver>
/// <type_tag ptr+len>
/// <field ptr+len>          ; e.g. "annotations"
/// <compile arg0>           ; runtime map key
/// <compile arg1>           ; runtime value
/// call cel_builder_map_entry
/// ```
pub fn compile_builder_map_entry(
    step: &BuilderStep,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let (state_key, output_type) = match step {
        BuilderStep::MapEntry {
            state_key,
            output_type,
            ..
        } => (state_key.as_str(), output_type.as_str()),
        _ => anyhow::bail!("compile_builder_map_entry called with non-MapEntry step"),
    };

    let receiver = call_expr.target.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "Builder MapEntry step '{}' requires a receiver",
            call_expr.func_name
        )
    })?;

    if call_expr.args.len() != 2 {
        anyhow::bail!(
            "Builder MapEntry step '{}' expects 2 arguments, got {}",
            call_expr.func_name,
            call_expr.args.len()
        );
    }

    let mem = get_memory_id(module)?;

    // receiver (existing map)
    compile_expr(&receiver.expr, body, env, ctx, module)?;

    // type tag
    emit_string_const(output_type, body, env, mem, module);

    // field name (compile-time constant)
    emit_string_const(state_key, body, env, mem, module);

    // arg0: runtime map key
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;

    // arg1: runtime value
    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;

    body.call(env.get(RuntimeFunction::BuilderMapEntryCall));
    Ok(())
}

// ─── Terminal step ────────────────────────────────────────────────────────────

/// Compile a builder **Terminal** step.
///
/// Terminal steps emit a host extension call ([`RuntimeFunction::ExtCall1`])
/// with the accumulated state map (and extra arguments folded in).
///
/// For `.list()` (no extra args):
/// ```text
/// <compile receiver>
/// call ExtCall1  ; host_namespace="kw.k8s", host_function="list"
/// ```
///
/// For `.get("nginx")` (`extra_arg_keys = ["name"]`):
/// ```text
/// <compile receiver>            ; existing map
/// <type_tag ptr+len>            ; same as input_type
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
    let (extra_arg_keys, host_namespace, host_function, input_type) = match step {
        BuilderStep::Terminal {
            extra_arg_keys,
            host_namespace,
            host_function,
            input_type,
            ..
        } => (
            extra_arg_keys,
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

    if call_expr.args.len() != extra_arg_keys.len() {
        anyhow::bail!(
            "Builder terminal '{}' expects {} argument(s), got {}",
            call_expr.func_name,
            extra_arg_keys.len(),
            call_expr.args.len()
        );
    }

    let mem = get_memory_id(module)?;

    // Compile the receiver (base state map).
    compile_expr(&receiver.expr, body, env, ctx, module)?;

    // Fold each extra argument into the map via cel_builder_step.
    for (i, key) in extra_arg_keys.iter().enumerate() {
        emit_string_const(input_type, body, env, mem, module); // keep same __type__
        emit_string_const(key, body, env, mem, module);
        compile_expr(&call_expr.args[i].expr, body, env, ctx, module)?;
        body.i32_const(0); // accumulate = false
        body.call(env.get(RuntimeFunction::BuilderStepCall));
    }

    // Now emit the host ExtCall1: (ns_ptr, ns_len, fn_ptr, fn_len, map_ptr) → *CelValue
    // The map is on the stack; we need ns/fn *before* it in the call frame.
    // Stash the map pointer in a local, push ns/fn, then restore it.
    let map_local = module.locals.add(walrus::ValType::I32);
    body.local_set(map_local);

    emit_string_const(host_namespace, body, env, mem, module);
    emit_string_const(host_function, body, env, mem, module);
    body.local_get(map_local);

    body.call(env.get(RuntimeFunction::ExtCall1));
    Ok(())
}

// ─── Compile-time type tracking ──────────────────────────────────────────────

/// Resolve the static builder `output_type` of an expression, if it is a
/// builder chain expression.
///
/// This walks the AST to determine the `__type__` tag that the expression
/// would produce at runtime.  Used by [`try_compile_builder_call`] to
/// disambiguate steps registered under the same method name.
///
/// Returns `None` for non-builder expressions or when the type cannot be
/// determined.
fn compile_type_of(expr: &Expr, ctx: &CompilerContext) -> Option<String> {
    match expr {
        Expr::Call(call) => {
            // Try Entry first (full dotted name).
            let full_name = build_full_name_from_call(call);
            if let Some(entry) = ctx.extensions.builder_entries.get(&full_name) {
                return entry.output_type().map(|s| s.to_string());
            }

            // Try Chain / MapEntry / Terminal (short method name with receiver).
            if let Some(target) = &call.target {
                let receiver_ty = compile_type_of(&target.expr, ctx);
                let short = &call.func_name;
                if let Some(steps) = ctx.extensions.builder_steps.get(short.as_str()) {
                    let matched =
                        find_matching_step(steps, receiver_ty.as_deref(), call.args.len());
                    if let Some(step) = matched {
                        return step.output_type().map(|s| s.to_string());
                    }
                }
            }

            None
        }
        _ => None,
    }
}

/// Reconstruct the full dotted name from a CallExpr (for Entry lookup).
fn build_full_name_from_call(call: &CallExpr) -> String {
    let mut segments: Vec<&str> = Vec::new();
    collect_segments(call.target.as_deref().map(|e| &e.expr), &mut segments);
    segments.push(&call.func_name);
    segments.join(".")
}

fn collect_segments<'a>(expr: Option<&'a Expr>, out: &mut Vec<&'a str>) {
    match expr {
        Some(Expr::Ident(name)) => out.push(name.as_str()),
        Some(Expr::Select(sel)) => {
            collect_segments(Some(&sel.operand.expr), out);
            out.push(&sel.field);
        }
        _ => {}
    }
}

// ─── Step selection ──────────────────────────────────────────────────────────

/// Find the unique step matching the receiver type and argument count.
///
/// Returns `Some(step)` if exactly one candidate matches, `None` otherwise.
fn find_matching_step<'a>(
    steps: &'a [BuilderStep],
    receiver_ty: Option<&str>,
    arg_count: usize,
) -> Option<&'a BuilderStep> {
    // First try: filter by both input_type and arity.
    if let Some(ty) = receiver_ty {
        let candidates: Vec<&BuilderStep> = steps
            .iter()
            .filter(|s| s.input_type() == Some(ty) && s.expected_args() == arg_count)
            .collect();
        if candidates.len() == 1 {
            return Some(candidates[0]);
        }
    }

    // Fallback: filter by arity alone (receiver type unknown).
    let by_arity: Vec<&BuilderStep> = steps
        .iter()
        .filter(|s| s.expected_args() == arg_count)
        .collect();
    if by_arity.len() == 1 {
        return Some(by_arity[0]);
    }

    None
}

// ─── Dispatch helper ──────────────────────────────────────────────────────────

/// Find the best matching builder step for a given call and dispatch to the
/// appropriate compile function.
///
/// For **Entry** steps the lookup key is the full dotted function name
/// (e.g. `"kw.k8s.apiVersion"`).  For **Chain / MapEntry / Terminal** steps
/// the lookup key is the short method name (e.g. `"kind"`), disambiguated
/// by the receiver's static type and the call's argument count.
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

    // 2. Check Chain / MapEntry / Terminal steps (keyed by short method name).
    //    Only apply when the call has a receiver (target).
    if let Some(target) = &call_expr.target {
        let short = &call_expr.func_name;
        if let Some(steps) = ctx.extensions.builder_steps.get(short.as_str()) {
            // Resolve the receiver's static type for disambiguation.
            let receiver_ty = compile_type_of(&target.expr, ctx);
            let step = find_matching_step(steps, receiver_ty.as_deref(), call_expr.args.len())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "No matching builder step '{}' for receiver type {:?} with {} arg(s)",
                        short,
                        receiver_ty,
                        call_expr.args.len()
                    )
                })?;

            match step {
                BuilderStep::Chain { .. } => {
                    compile_builder_chain(step, call_expr, body, env, ctx, module)?;
                }
                BuilderStep::MapEntry { .. } => {
                    compile_builder_map_entry(step, call_expr, body, env, ctx, module)?;
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
