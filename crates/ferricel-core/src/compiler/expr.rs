use cel::common::ast::{CallExpr, Expr};
use walrus::InstrSeqBuilder;

use crate::compiler::{
    access, collections,
    context::{CompilerContext, CompilerEnv},
    functions, literals, operators,
};

/// The main recursive expression compiler.
///
/// Always leaves a *mut CelValue (i32) on the stack.
pub fn compile_expr(
    expr: &Expr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match expr {
        Expr::Literal(literal) => literals::compile_literal(literal, body, env, module)?,

        Expr::Call(call_expr) => compile_call(call_expr, body, env, ctx, module)?,

        Expr::Ident(name) => access::compile_ident(name, body, env, ctx, module)?,

        Expr::Select(sel) => access::compile_select(sel, body, env, ctx, module)?,

        Expr::List(list) => collections::compile_list(list, body, env, ctx, module)?,

        Expr::Map(map) => collections::compile_map(map, body, env, ctx, module)?,

        Expr::Comprehension(comp) => {
            collections::compile_comprehension(comp, body, env, ctx, module)?
        }

        Expr::Struct(s) => collections::compile_struct(s, body, env, ctx, module)?,

        _ => anyhow::bail!("Unsupported expression type: {:?}", expr),
    }

    Ok(())
}

/// Dispatch a `Call` expression to either the operators module or the named-function module.
fn compile_call(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let func_name = call_expr.func_name.as_str();

    // Try operators first (they have fixed names starting with "_")
    if operators::compile_operator(func_name, call_expr, body, env, ctx, module)? {
        return Ok(());
    }

    // Fall through to named functions
    functions::compile_named_function(func_name, call_expr, body, env, ctx, module)
}
