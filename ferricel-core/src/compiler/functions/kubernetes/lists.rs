//! Compiler support for Kubernetes CEL list library extensions.
//!
//! Dispatches the 6 Kubernetes-specific list methods to their runtime
//! counterparts:
//!   - `isSorted`     → `cel_k8s_list_is_sorted`
//!   - `sum`          → `cel_k8s_list_sum`
//!   - `min`          → `cel_k8s_list_min`
//!   - `max`          → `cel_k8s_list_max`
//!   - `indexOf`      → `cel_k8s_list_index_of`
//!   - `lastIndexOf`  → `cel_k8s_list_last_index_of`

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    helpers::{compile_call_binary, compile_call_unary},
};

/// Compile a Kubernetes list extension method call.
pub fn compile_k8s_list_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        "isSorted" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sListIsSorted,
            body,
            env,
            ctx,
            module,
        ),
        "sum" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sListSum,
            body,
            env,
            ctx,
            module,
        ),
        "min" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sListMin,
            body,
            env,
            ctx,
            module,
        ),
        "max" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sListMax,
            body,
            env,
            ctx,
            module,
        ),
        "indexOf" => compile_call_binary(
            call_expr,
            func_name,
            RuntimeFunction::K8sListIndexOf,
            body,
            env,
            ctx,
            module,
        ),
        "lastIndexOf" => compile_call_binary(
            call_expr,
            func_name,
            RuntimeFunction::K8sListLastIndexOf,
            body,
            env,
            ctx,
            module,
        ),
        _ => anyhow::bail!("Unknown Kubernetes list function: {}", func_name),
    }
}
