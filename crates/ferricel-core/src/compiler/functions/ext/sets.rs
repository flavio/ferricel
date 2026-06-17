//! Compiler for CEL sets extension library functions.
//!
//! Handles:
//! - `sets.contains(list, sublist) -> bool`
//! - `sets.intersects(listA, listB) -> bool`
//! - `sets.equivalent(listA, listB) -> bool`
//!
//! All three are namespace-qualified global functions. The caller in `functions/mod.rs`
//! already guards on the `sets` namespace before dispatching here.
//!
//! CEL parses `sets.contains(a, b)` as:
//!   target = Ident("sets"), func = "contains", args = [a, b]

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
};

/// Compile a sets extension function call.
pub fn compile_ext_sets_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        "contains" => compile_sets_contains(call_expr, body, env, ctx, module),
        "intersects" => compile_sets_intersects(call_expr, body, env, ctx, module),
        "equivalent" => compile_sets_equivalent(call_expr, body, env, ctx, module),
        _ => anyhow::bail!("Unknown sets function: {}", func_name),
    }
}

/// Compile `sets.contains(list, sublist) -> bool`.
fn compile_sets_contains(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 2 {
        anyhow::bail!("sets.contains() expects exactly 2 arguments");
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
    body.call(env.get(RuntimeFunction::SetsContains));
    Ok(())
}

/// Compile `sets.intersects(listA, listB) -> bool`.
fn compile_sets_intersects(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 2 {
        anyhow::bail!("sets.intersects() expects exactly 2 arguments");
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
    body.call(env.get(RuntimeFunction::SetsIntersects));
    Ok(())
}

/// Compile `sets.equivalent(listA, listB) -> bool`.
fn compile_sets_equivalent(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 2 {
        anyhow::bail!("sets.equivalent() expects exactly 2 arguments");
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
    body.call(env.get(RuntimeFunction::SetsEquivalent));
    Ok(())
}
