//! Compiler for CEL extended list library functions.
//!
//! Handles: `join`, `distinct`, `flatten`, `reverse`, `slice`, `sort`.
//! `lists.range(n)` is dispatched from `mod.rs` with a namespace guard and
//! compiled here via `compile_list_range`.

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::{compile_call_binary, compile_call_ternary, compile_call_unary},
};

/// Compile an extended list library method call (receiver-style).
pub fn compile_ext_list_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        "join" => compile_join(call_expr, body, env, ctx, module),
        "distinct" => compile_call_unary(
            call_expr,
            "distinct",
            RuntimeFunction::ListDistinct,
            body,
            env,
            ctx,
            module,
        ),
        "flatten" => compile_flatten(call_expr, body, env, ctx, module),
        "reverse" => compile_call_unary(
            call_expr,
            "reverse",
            RuntimeFunction::ListReverse,
            body,
            env,
            ctx,
            module,
        ),
        "slice" => compile_call_ternary(
            call_expr,
            "slice",
            RuntimeFunction::ListSlice,
            body,
            env,
            ctx,
            module,
        ),
        "sort" => compile_call_unary(
            call_expr,
            "sort",
            RuntimeFunction::ListSort,
            body,
            env,
            ctx,
            module,
        ),
        _ => anyhow::bail!("Unknown ext list function: {}", func_name),
    }
}

/// Compile `lists.range(n)` — namespace-qualified, no receiver.
///
/// CEL parses `lists.range(n)` as:
///   target = Ident("lists"), func = "range", args = [n]
pub fn compile_list_range(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 1 {
        anyhow::bail!("lists.range() expects exactly 1 argument");
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    body.call(env.get(RuntimeFunction::ListRange));
    Ok(())
}

/// Compile `list.join()` or `list.join(sep)`.
fn compile_join(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let arg_count = if call_expr.target.is_some() {
        call_expr.args.len()
    } else {
        call_expr.args.len().saturating_sub(1)
    };

    match arg_count {
        0 => compile_call_unary(
            call_expr,
            "join",
            RuntimeFunction::ListJoin,
            body,
            env,
            ctx,
            module,
        ),
        1 => compile_call_binary(
            call_expr,
            "join",
            RuntimeFunction::ListJoinSep,
            body,
            env,
            ctx,
            module,
        ),
        _ => anyhow::bail!("join() expects 0 or 1 arguments"),
    }
}

/// Compile `list.flatten()` or `list.flatten(depth)`.
fn compile_flatten(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let arg_count = if call_expr.target.is_some() {
        call_expr.args.len()
    } else {
        call_expr.args.len().saturating_sub(1)
    };

    match arg_count {
        0 => compile_call_unary(
            call_expr,
            "flatten",
            RuntimeFunction::ListFlatten,
            body,
            env,
            ctx,
            module,
        ),
        1 => compile_call_binary(
            call_expr,
            "flatten",
            RuntimeFunction::ListFlattenDepth,
            body,
            env,
            ctx,
            module,
        ),
        _ => anyhow::bail!("flatten() expects 0 or 1 arguments"),
    }
}
