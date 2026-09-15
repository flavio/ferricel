//! Compiler for core CEL string functions.
//!
//! Handles: `size`, `startsWith`, `endsWith`, `contains`, `matches`.
//! Extended string library functions live in `ext/strings.rs`.

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::compile_call_binary,
};

/// Compile a core string function call.
pub fn compile_string_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        "size" => compile_size(call_expr, body, env, ctx, module),
        "startsWith" => compile_call_binary(
            call_expr,
            "startsWith",
            RuntimeFunction::StringStartsWith,
            body,
            env,
            ctx,
            module,
        ),
        "endsWith" => compile_call_binary(
            call_expr,
            "endsWith",
            RuntimeFunction::StringEndsWith,
            body,
            env,
            ctx,
            module,
        ),
        "contains" => compile_call_binary(
            call_expr,
            "contains",
            RuntimeFunction::StringContains,
            body,
            env,
            ctx,
            module,
        ),
        "matches" => compile_call_binary(
            call_expr,
            "matches",
            RuntimeFunction::StringMatches,
            body,
            env,
            ctx,
            module,
        ),
        _ => anyhow::bail!("Unknown string function: {}", func_name),
    }
}

/// Compile `size()` which works on strings, bytes, arrays, or maps.
///
/// Supports both forms:
/// - `size(x)` — free function with 1 arg
/// - `x.size()` — method with target and 0 args
fn compile_size(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match (call_expr.args.len(), &call_expr.target) {
        (1, _) => {
            // size(x) — free function form
            compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
        }
        (0, Some(target)) => {
            // x.size() — method form
            compile_expr(&target.expr, body, env, ctx, module)?;
        }
        _ => anyhow::bail!("size() expects 1 argument"),
    }

    // `cel_value_size` returns a `*mut CelValue`: `CelValue::Int` on success,
    // or `CelValue::Error` for a non-collection value (propagated or fresh
    // `no such overload`).
    body.call(env.get(RuntimeFunction::ValueSize));
    Ok(())
}
