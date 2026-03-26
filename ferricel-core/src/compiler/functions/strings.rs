use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::compile_call_binary,
};

/// Compile a string function call.
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
fn compile_size(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 1 {
        anyhow::bail!("size() expects 1 argument");
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;

    // Call polymorphic cel_value_size which returns i64
    // We need to convert it to *mut CelValue::Int
    body.call(env.get(RuntimeFunction::ValueSize)); // Returns i64
    body.call(env.get(RuntimeFunction::CreateInt)); // Convert i64 to *mut CelValue
    Ok(())
}
