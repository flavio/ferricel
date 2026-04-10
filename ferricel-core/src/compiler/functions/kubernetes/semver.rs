//! Compiler support for Kubernetes CEL semver library extensions.
//!
//! Dispatches the Kubernetes semver functions to their runtime counterparts:
//!   - `isSemver(string)`               → `cel_k8s_is_semver`               (unary, function-style)
//!   - `isSemver(string, bool)`          → `cel_k8s_is_semver_normalize`     (binary, function-style)
//!   - `semver(string)`                  → `cel_k8s_semver_parse`            (unary, function-style)
//!   - `semver(string, bool)`            → `cel_k8s_semver_parse_normalize`  (binary, function-style)
//!   - `<Semver>.major()`                → `cel_k8s_semver_major`            (unary, method-style)
//!   - `<Semver>.minor()`                → `cel_k8s_semver_minor`            (unary, method-style)
//!   - `<Semver>.patch()`                → `cel_k8s_semver_patch`            (unary, method-style)
//!   - `<Semver>.isLessThan(Semver)`     → `cel_k8s_semver_is_less_than`     (binary, method-style)
//!   - `<Semver>.isGreaterThan(Semver)`  → `cel_k8s_semver_is_greater_than`  (binary, method-style)
//!   - `<Semver>.compareTo(Semver)`      → `cel_k8s_semver_compare_to`       (binary, method-style)
//!
//! Reference: <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-semver-library>

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::{compile_call_binary, compile_call_unary},
};

/// Compile a Kubernetes semver extension function/method call.
pub fn compile_k8s_semver_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        // isSemver: unary or binary (with normalize flag)
        "isSemver" => {
            // Both function-style and method-style; arity determines the variant.
            let n_args = if call_expr.target.is_some() {
                call_expr.args.len() + 1 // target counts as one argument
            } else {
                call_expr.args.len()
            };

            if n_args == 1 {
                // isSemver(str)
                compile_call_unary(
                    call_expr,
                    func_name,
                    RuntimeFunction::K8sSemverIsSemver,
                    body,
                    env,
                    ctx,
                    module,
                )
            } else if n_args == 2 {
                // isSemver(str, normalize)
                compile_call_binary(
                    call_expr,
                    func_name,
                    RuntimeFunction::K8sSemverIsSemverNormalize,
                    body,
                    env,
                    ctx,
                    module,
                )
            } else {
                anyhow::bail!("isSemver() expects 1 or 2 arguments, got {}", n_args)
            }
        }

        // semver: unary or binary (with normalize flag)
        "semver" => {
            let n_args = if call_expr.target.is_some() {
                call_expr.args.len() + 1
            } else {
                call_expr.args.len()
            };

            if n_args == 1 {
                // semver(str)
                compile_call_unary(
                    call_expr,
                    func_name,
                    RuntimeFunction::K8sSemverParse,
                    body,
                    env,
                    ctx,
                    module,
                )
            } else if n_args == 2 {
                // semver(str, normalize)
                //
                // Function-style: semver(str, bool) — target=None, args=[str, bool]
                // We compile both args and call the binary runtime function.
                if call_expr.target.is_none() && call_expr.args.len() == 2 {
                    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
                    body.call(env.get(RuntimeFunction::K8sSemverParseNormalize));
                    Ok(())
                } else {
                    anyhow::bail!("semver(str, bool) must be called as a function, not a method")
                }
            } else {
                anyhow::bail!("semver() expects 1 or 2 arguments, got {}", n_args)
            }
        }

        // Unary method-style: <Semver>.major(), <Semver>.minor(), <Semver>.patch()
        "major" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sSemverMajor,
            body,
            env,
            ctx,
            module,
        ),
        "minor" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sSemverMinor,
            body,
            env,
            ctx,
            module,
        ),
        "patch" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sSemverPatch,
            body,
            env,
            ctx,
            module,
        ),

        // Binary method-style: <Semver>.isLessThan(other), isGreaterThan(other), compareTo(other)
        "isLessThan" => compile_call_binary(
            call_expr,
            func_name,
            RuntimeFunction::K8sSemverIsLessThan,
            body,
            env,
            ctx,
            module,
        ),
        "isGreaterThan" => compile_call_binary(
            call_expr,
            func_name,
            RuntimeFunction::K8sSemverIsGreaterThan,
            body,
            env,
            ctx,
            module,
        ),
        "compareTo" => compile_call_binary(
            call_expr,
            func_name,
            RuntimeFunction::K8sSemverCompareTo,
            body,
            env,
            ctx,
            module,
        ),

        _ => anyhow::bail!("Unknown Kubernetes semver function: {}", func_name),
    }
}
