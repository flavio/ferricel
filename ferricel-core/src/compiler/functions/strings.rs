use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
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
        "startsWith" => compile_method_or_function(
            call_expr,
            "startsWith",
            RuntimeFunction::StringStartsWith,
            body,
            env,
            ctx,
            module,
        ),
        "endsWith" => compile_method_or_function(
            call_expr,
            "endsWith",
            RuntimeFunction::StringEndsWith,
            body,
            env,
            ctx,
            module,
        ),
        "contains" => compile_method_or_function(
            call_expr,
            "contains",
            RuntimeFunction::StringContains,
            body,
            env,
            ctx,
            module,
        ),
        "matches" => compile_method_or_function(
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

/// Handle the shared method-or-function dispatch pattern used by startsWith, endsWith,
/// contains, and matches.
///
/// - Method syntax: `target.func(arg)` - target is Some, args has 1 element
/// - Function syntax: `func(str, arg)` - target is None, args has 2 elements
fn compile_method_or_function(
    call_expr: &CallExpr,
    func_name: &str,
    runtime_fn: RuntimeFunction,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if let Some(target) = &call_expr.target {
        // Method syntax: "hello".startsWith("he")
        if call_expr.args.len() != 1 {
            anyhow::bail!("{}() method expects 1 argument", func_name);
        }
        compile_expr(&target.expr, body, env, ctx, module)?;
        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    } else {
        // Function syntax: startsWith("hello", "he")
        if call_expr.args.len() != 2 {
            anyhow::bail!("{}() function expects 2 arguments", func_name);
        }
        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
        compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
    }
    body.call(env.get(runtime_fn));
    Ok(())
}
