//! Compiler support for Kubernetes CEL regex library extensions.
//!
//! Dispatches the Kubernetes-specific regex methods to their runtime
//! counterparts:
//!   - `find`    (1 arg)  → `cel_k8s_regex_find`
//!   - `findAll` (1 arg)  → `cel_k8s_regex_find_all_n` with limit = -1
//!   - `findAll` (2 args) → `cel_k8s_regex_find_all_n`

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::{compile_call_binary, compile_call_ternary},
};

pub fn compile_k8s_regex_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        "find" => compile_call_binary(
            call_expr,
            func_name,
            RuntimeFunction::K8sRegexFind,
            body,
            env,
            ctx,
            module,
        ),
        "findAll" => {
            // Determine which overload to use based on argument count.
            // Method style: receiver.findAll(pattern) or receiver.findAll(pattern, limit)
            // Function style: findAll(str, pattern) or findAll(str, pattern, limit)
            let arg_count = if call_expr.target.is_some() {
                call_expr.args.len()
            } else {
                // function style: first arg is receiver, rest are parameters
                call_expr.args.len().saturating_sub(1)
            };

            match arg_count {
                1 => {
                    // No limit supplied — compile as findAll_n(str, pattern, -1).
                    // Emit the two positional args (receiver + pattern), then push
                    // the literal -1 as a *mut CelValue, then call findAll_n.
                    if let Some(target) = &call_expr.target {
                        if call_expr.args.len() != 1 {
                            anyhow::bail!("findAll() method expects 1 argument");
                        }
                        compile_expr(&target.expr, body, env, ctx, module)?;
                        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    } else {
                        if call_expr.args.len() != 2 {
                            anyhow::bail!("findAll() function expects 2 arguments");
                        }
                        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                        compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    }
                    // Inject limit = -1
                    body.i64_const(-1);
                    body.call(env.get(RuntimeFunction::CreateInt));
                    body.call(env.get(RuntimeFunction::K8sRegexFindAllN));
                    Ok(())
                }
                2 => compile_call_ternary(
                    call_expr,
                    func_name,
                    RuntimeFunction::K8sRegexFindAllN,
                    body,
                    env,
                    ctx,
                    module,
                ),
                n => anyhow::bail!("findAll() expects 1 or 2 arguments, got {}", n),
            }
        }
        _ => anyhow::bail!("Unknown Kubernetes regex function: {}", func_name),
    }
}
