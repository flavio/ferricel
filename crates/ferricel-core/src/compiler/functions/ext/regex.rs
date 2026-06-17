//! Compiler for CEL regex extension library functions.
//!
//! Handles:
//!   - `regex.replace(target, pattern, replacement) -> string`
//!   - `regex.replace(target, pattern, replacement, count) -> string`
//!   - `regex.extract(target, pattern) -> optional<string>`
//!   - `regex.extractAll(target, pattern) -> list<string>`
//!
//! CEL parses `regex.foo(a, b, c)` as:
//!   target = Ident("regex"), func = "foo", args = [a, b, c]
//!
//! So all arguments are in `call_expr.args`; `call_expr.target` is the `regex`
//! namespace ident and carries no runtime value.  We compile args directly
//! rather than using the generic `compile_call_*` helpers (which assume the
//! target is a real runtime receiver).

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
};

/// Compile a regex extension function call.
pub fn compile_ext_regex_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        "replace" => compile_regex_replace(call_expr, body, env, ctx, module),
        "extract" => compile_regex_binary(
            call_expr,
            "regex.extract",
            RuntimeFunction::RegexExtract,
            body,
            env,
            ctx,
            module,
        ),
        "extractAll" => compile_regex_binary(
            call_expr,
            "regex.extractAll",
            RuntimeFunction::RegexExtractAll,
            body,
            env,
            ctx,
            module,
        ),
        _ => anyhow::bail!("Unknown regex extension function: {}", func_name),
    }
}

/// Compile a 2-argument regex function: `regex.foo(target, pattern)`.
///
/// Both arguments are in `call_expr.args[0]` and `call_expr.args[1]`.
fn compile_regex_binary(
    call_expr: &CallExpr,
    func_name: &str,
    runtime_fn: RuntimeFunction,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 2 {
        anyhow::bail!(
            "{}() expects 2 arguments, got {}",
            func_name,
            call_expr.args.len()
        );
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
    body.call(env.get(runtime_fn));
    Ok(())
}

/// Compile `regex.replace` — overloaded by arity:
///   - 3 args (target, pattern, replacement)        → `cel_regex_replace`
///   - 4 args (target, pattern, replacement, count)  → `cel_regex_replace_n`
///
/// All arguments are in `call_expr.args`; `regex` is the target ident (not a
/// real runtime receiver).
fn compile_regex_replace(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match call_expr.args.len() {
        3 => {
            compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
            compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
            compile_expr(&call_expr.args[2].expr, body, env, ctx, module)?;
            body.call(env.get(RuntimeFunction::RegexReplace));
            Ok(())
        }
        4 => {
            compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
            compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
            compile_expr(&call_expr.args[2].expr, body, env, ctx, module)?;
            compile_expr(&call_expr.args[3].expr, body, env, ctx, module)?;
            body.call(env.get(RuntimeFunction::RegexReplaceN));
            Ok(())
        }
        n => anyhow::bail!("regex.replace() expects 3 or 4 arguments, got {}", n),
    }
}
