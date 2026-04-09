//! Compiler for polymorphic `indexOf` / `lastIndexOf`.
//!
//! These functions are overloaded by arity:
//! - 1 extra arg  → polymorphic runtime function (handles both string and list receivers)
//! - 2 extra args → string-only version with an offset parameter

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    helpers::{compile_call_binary, compile_call_ternary},
};

/// Dispatch `indexOf` and `lastIndexOf` based on argument count.
pub fn compile_index_of_dispatch(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let extra_args = if call_expr.target.is_some() {
        call_expr.args.len()
    } else {
        call_expr.args.len().saturating_sub(1)
    };

    match (func_name, extra_args) {
        ("indexOf", 1) => compile_call_binary(
            call_expr,
            "indexOf",
            RuntimeFunction::IndexOfPoly,
            body,
            env,
            ctx,
            module,
        ),
        ("indexOf", 2) => compile_call_ternary(
            call_expr,
            "indexOf",
            RuntimeFunction::StringIndexOfOffset,
            body,
            env,
            ctx,
            module,
        ),
        ("lastIndexOf", 1) => compile_call_binary(
            call_expr,
            "lastIndexOf",
            RuntimeFunction::LastIndexOfPoly,
            body,
            env,
            ctx,
            module,
        ),
        ("lastIndexOf", 2) => compile_call_ternary(
            call_expr,
            "lastIndexOf",
            RuntimeFunction::StringLastIndexOfOffset,
            body,
            env,
            ctx,
            module,
        ),
        _ => anyhow::bail!("{func_name}() expects 1 or 2 arguments"),
    }
}
