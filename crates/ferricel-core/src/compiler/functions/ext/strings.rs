//! Compiler for CEL extended string library functions.
//!
//! Handles: `lowerAscii`, `upperAscii`, `trim`, `reverse`, `charAt`,
//! `replace`, `split`, `substring`, `format`, `strings.quote`.

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::{
        compile_call_binary, compile_call_quaternary, compile_call_ternary, compile_call_unary,
    },
};

/// Compile an extended string library function call.
pub fn compile_ext_string_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        // Unary
        "lowerAscii" => compile_call_unary(
            call_expr,
            "lowerAscii",
            RuntimeFunction::StringLowerAscii,
            body,
            env,
            ctx,
            module,
        ),
        "upperAscii" => compile_call_unary(
            call_expr,
            "upperAscii",
            RuntimeFunction::StringUpperAscii,
            body,
            env,
            ctx,
            module,
        ),
        "trim" => compile_call_unary(
            call_expr,
            "trim",
            RuntimeFunction::StringTrim,
            body,
            env,
            ctx,
            module,
        ),
        "reverse" => compile_call_unary(
            call_expr,
            "reverse",
            RuntimeFunction::StringReverse,
            body,
            env,
            ctx,
            module,
        ),

        // Binary
        "charAt" => compile_call_binary(
            call_expr,
            "charAt",
            RuntimeFunction::StringCharAt,
            body,
            env,
            ctx,
            module,
        ),
        "format" => compile_call_binary(
            call_expr,
            "format",
            RuntimeFunction::StringFormat,
            body,
            env,
            ctx,
            module,
        ),

        // Arity-overloaded
        "replace" => compile_replace(call_expr, body, env, ctx, module),
        "split" => compile_split(call_expr, body, env, ctx, module),
        "substring" => compile_substring(call_expr, body, env, ctx, module),

        // Namespace function
        "quote" => compile_strings_quote(call_expr, body, env, ctx, module),

        _ => anyhow::bail!("Unknown ext string function: {}", func_name),
    }
}

/// Compile `replace(old, new)` or `replace(old, new, n)`.
fn compile_replace(
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
        2 => compile_call_ternary(
            call_expr,
            "replace",
            RuntimeFunction::StringReplace,
            body,
            env,
            ctx,
            module,
        ),
        3 => compile_call_quaternary(
            call_expr,
            "replace",
            RuntimeFunction::StringReplaceN,
            body,
            env,
            ctx,
            module,
        ),
        _ => anyhow::bail!("replace() expects 2 or 3 arguments"),
    }
}

/// Compile `split(sep)` or `split(sep, n)`.
fn compile_split(
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
        1 => compile_call_binary(
            call_expr,
            "split",
            RuntimeFunction::StringSplit,
            body,
            env,
            ctx,
            module,
        ),
        2 => compile_call_ternary(
            call_expr,
            "split",
            RuntimeFunction::StringSplitN,
            body,
            env,
            ctx,
            module,
        ),
        _ => anyhow::bail!("split() expects 1 or 2 arguments"),
    }
}

/// Compile `substring(start)` or `substring(start, end)`.
fn compile_substring(
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
        1 => compile_call_binary(
            call_expr,
            "substring",
            RuntimeFunction::StringSubstring,
            body,
            env,
            ctx,
            module,
        ),
        2 => compile_call_ternary(
            call_expr,
            "substring",
            RuntimeFunction::StringSubstringRange,
            body,
            env,
            ctx,
            module,
        ),
        _ => anyhow::bail!("substring() expects 1 or 2 arguments"),
    }
}

/// Compile `strings.quote(s)` — a global function in the "strings" namespace.
///
/// CEL parses `strings.quote("hello")` as: target=strings ident, func=quote, args=["hello"].
/// We ignore the namespace target and compile the single argument.
fn compile_strings_quote(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    // strings.quote is always called with one explicit string argument in args
    if call_expr.args.len() != 1 {
        anyhow::bail!("strings.quote() expects exactly 1 argument");
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    body.call(env.get(RuntimeFunction::StringsQuote));
    Ok(())
}
