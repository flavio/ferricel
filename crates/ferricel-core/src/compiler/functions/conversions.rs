use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
};

/// Compile a type conversion function call.
pub fn compile_conversion_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        "string" => compile_unary_fn(
            call_expr,
            "string",
            RuntimeFunction::String,
            body,
            env,
            ctx,
            module,
        ),
        "int" => compile_unary_fn(
            call_expr,
            "int",
            RuntimeFunction::Int,
            body,
            env,
            ctx,
            module,
        ),
        "uint" => compile_unary_fn(
            call_expr,
            "uint",
            RuntimeFunction::Uint,
            body,
            env,
            ctx,
            module,
        ),
        "double" => compile_unary_fn(
            call_expr,
            "double",
            RuntimeFunction::Double,
            body,
            env,
            ctx,
            module,
        ),
        "bytes" => compile_unary_fn(
            call_expr,
            "bytes",
            RuntimeFunction::Bytes,
            body,
            env,
            ctx,
            module,
        ),
        "bool" => compile_unary_fn(
            call_expr,
            "bool",
            RuntimeFunction::Bool,
            body,
            env,
            ctx,
            module,
        ),
        "type" => compile_unary_fn(
            call_expr,
            "type",
            RuntimeFunction::Type,
            body,
            env,
            ctx,
            module,
        ),
        "dyn" => {
            // dyn(value) - identity function that marks value as dynamically typed
            // In CEL, this is used to force dynamic dispatch for operations
            // For our compiler, it's a no-op since we already do dynamic dispatch
            if call_expr.args.len() != 1 {
                anyhow::bail!("dyn() expects 1 argument");
            }
            compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
            // No function call needed - just leave the value on the stack
            Ok(())
        }
        _ => anyhow::bail!("Unknown conversion function: {}", func_name),
    }
}

/// Compile a simple 1-arg function: validate 1 arg, compile it, call the runtime function.
fn compile_unary_fn(
    call_expr: &CallExpr,
    func_name: &str,
    runtime_fn: RuntimeFunction,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 1 {
        anyhow::bail!("{}() expects 1 argument", func_name);
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    body.call(env.get(runtime_fn));
    Ok(())
}
