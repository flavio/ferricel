//! Compiler for CEL encoder extension library functions.
//!
//! Handles: `base64.encode(bytes) -> string`, `base64.decode(string) -> bytes`.
//!
//! Both are namespace-qualified global functions; the caller in `functions/mod.rs`
//! already guards on the `base64` namespace before dispatching here.

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
};

/// Compile an encoder extension function call (`base64.encode` or `base64.decode`).
pub fn compile_ext_encoder_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        "encode" => compile_base64_encode(call_expr, body, env, ctx, module),
        "decode" => compile_base64_decode(call_expr, body, env, ctx, module),
        _ => anyhow::bail!("Unknown encoder function: {}", func_name),
    }
}

/// Compile `base64.encode(bytes_arg) -> string`.
///
/// CEL parses `base64.encode(b'hello')` as:
///   target = Ident("base64"), func = "encode", args = [b'hello']
fn compile_base64_encode(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 1 {
        anyhow::bail!("base64.encode() expects exactly 1 argument");
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    body.call(env.get(RuntimeFunction::Base64Encode));
    Ok(())
}

/// Compile `base64.decode(string_arg) -> bytes`.
///
/// CEL parses `base64.decode('aGVsbG8=')` as:
///   target = Ident("base64"), func = "decode", args = ['aGVsbG8=']
fn compile_base64_decode(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 1 {
        anyhow::bail!("base64.decode() expects exactly 1 argument");
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    body.call(env.get(RuntimeFunction::Base64Decode));
    Ok(())
}
