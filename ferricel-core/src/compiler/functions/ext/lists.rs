//! Compiler for CEL extended list library functions.
//!
//! Handles: `join` (with optional separator).

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    helpers::{compile_call_binary, compile_call_unary},
};

/// Compile an extended list library function call.
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
        _ => anyhow::bail!("Unknown ext list function: {}", func_name),
    }
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
