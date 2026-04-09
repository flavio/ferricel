//! Compiler support for Kubernetes CEL CIDR library extensions.
//!
//! Dispatches the Kubernetes CIDR functions to their runtime counterparts:
//!   - `isCIDR(string)`              → `cel_k8s_is_cidr`               (unary, function-style)
//!   - `cidr(string)`                → `cel_k8s_cidr_parse`             (unary, function-style)
//!   - `<CIDR>.ip()`                 → `cel_k8s_cidr_ip`                (unary, method-style)
//!   - `<CIDR>.masked()`             → `cel_k8s_cidr_masked`            (unary, method-style)
//!   - `<CIDR>.prefixLength()`       → `cel_k8s_cidr_prefix_length`     (unary, method-style)
//!   - `<CIDR>.containsIP(ip)`       → `cel_k8s_cidr_contains_ip_obj`   (binary, method-style)
//!   - `<CIDR>.containsCIDR(cidr)`   → `cel_k8s_cidr_contains_cidr_obj` (binary, method-style)
//!
//! Reference: <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-cidr-library>

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    helpers::{compile_call_binary, compile_call_unary},
};

/// Compile a Kubernetes CIDR extension function/method call.
pub fn compile_k8s_cidr_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        // Unary function-style: cidr(str), isCIDR(str)
        "cidr" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sCidrParse,
            body,
            env,
            ctx,
            module,
        ),
        "isCIDR" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sIsCidr,
            body,
            env,
            ctx,
            module,
        ),

        // Unary method-style: <CIDR>.ip(), <CIDR>.masked(), <CIDR>.prefixLength()
        //
        // Note: "ip" is also the IP address constructor (handled in ip.rs), but when called as a
        // method on a CIDR receiver (e.g. `cidr('...').ip()`), the CEL AST sets target=<cidr-expr>
        // and func_name="ip" with empty args. The routing in mod.rs sends "ip" here only when we
        // are already in CIDR context. In practice, the top-level dispatch in functions/mod.rs
        // routes "ip" to the IP module, so we handle it here via the caller passing the right module.
        // The CIDR compiler is only invoked from the cidr-specific arm in compile_named_function.
        "ip" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sCidrIp,
            body,
            env,
            ctx,
            module,
        ),
        "masked" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sCidrMasked,
            body,
            env,
            ctx,
            module,
        ),
        "prefixLength" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sCidrPrefixLength,
            body,
            env,
            ctx,
            module,
        ),

        // Binary method-style: <CIDR>.containsIP(ip_or_str), <CIDR>.containsCIDR(cidr_or_str)
        //
        // The runtime functions dispatch on the argument type at runtime, so we always emit
        // the `_obj` variant (which handles both IpAddr and String inputs).
        "containsIP" => compile_call_binary(
            call_expr,
            func_name,
            RuntimeFunction::K8sCidrContainsIpObj,
            body,
            env,
            ctx,
            module,
        ),
        "containsCIDR" => compile_call_binary(
            call_expr,
            func_name,
            RuntimeFunction::K8sCidrContainsCidrObj,
            body,
            env,
            ctx,
            module,
        ),

        _ => anyhow::bail!("Unknown Kubernetes CIDR function: {}", func_name),
    }
}
