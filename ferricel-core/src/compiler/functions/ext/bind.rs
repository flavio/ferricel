//! Compiler support for `cel.bind`.
//!
//! `cel.bind(var, init, body)` is a CEL extension macro that introduces a local
//! variable binding. It evaluates `init` once, binds the result to `var`, and
//! then evaluates `body` in that extended scope.
//!
//! The parser sees this as an ordinary function call:
//!   `Call { target: Ident("cel"), func: "bind", args: [Ident(var), init, body] }`
//!
//! Since there is no runtime function involved, this is compiled purely by
//! emitting a Wasm local for the initializer result and threading a child
//! `CompilerContext` that maps `var` to that local into the body compilation.
//!
//! Reference: <https://github.com/google/cel-go/blob/master/cel/library.go>

use cel::common::ast::{CallExpr, Expr};
use walrus::{InstrSeqBuilder, ValType};

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
};

/// Compile `cel.bind(var, init, body)`.
///
/// Steps:
/// 1. Validate: exactly 3 args, `args[0]` must be an identifier.
/// 2. Compile `init` → i32 pointer on the Wasm stack.
/// 3. Store in a fresh Wasm local.
/// 4. Build a child context with `var` mapped to that local.
/// 5. Compile `body` in the child context — its result is the result of the whole expression.
pub fn compile_cel_bind(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 3 {
        anyhow::bail!(
            "cel.bind() expects exactly 3 arguments (var, init, body), got {}",
            call_expr.args.len()
        );
    }

    let var_name = match &call_expr.args[0].expr {
        Expr::Ident(name) => name.clone(),
        _ => anyhow::bail!("cel.bind() first argument must be a simple identifier"),
    };

    // Compile the init expression; result pointer is left on the stack.
    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;

    // Store into a fresh local so the body can reference it by name.
    let local = module.locals.add(ValType::I32);
    body.local_set(local);

    // Compile the body in a child context where var_name resolves to the local.
    let child_ctx = ctx.with_local(var_name, local);
    compile_expr(&call_expr.args[2].expr, body, env, &child_ctx, module)
}
