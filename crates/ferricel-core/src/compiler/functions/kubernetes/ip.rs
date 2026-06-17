//! Compiler support for Kubernetes CEL IP address library extensions.
//!
//! Dispatches the Kubernetes IP address functions to their runtime counterparts:
//!   - `isIP(string)`                 → `cel_k8s_is_ip`                     (unary, function-style)
//!   - `ip(string)`                   → `cel_k8s_ip_parse`                   (unary, function-style)
//!   - `ip.isCanonical(string)`       → `cel_k8s_ip_is_canonical`            (namespace-unary: target=ip, args=[str])
//!   - `<IP>.family()`                → `cel_k8s_ip_family`                  (unary, method-style)
//!   - `<IP>.isUnspecified()`         → `cel_k8s_ip_is_unspecified`          (unary, method-style)
//!   - `<IP>.isLoopback()`            → `cel_k8s_ip_is_loopback`             (unary, method-style)
//!   - `<IP>.isLinkLocalMulticast()`  → `cel_k8s_ip_is_link_local_multicast` (unary, method-style)
//!   - `<IP>.isLinkLocalUnicast()`    → `cel_k8s_ip_is_link_local_unicast`   (unary, method-style)
//!   - `<IP>.isGlobalUnicast()`       → `cel_k8s_ip_is_global_unicast`       (unary, method-style)
//!
//! Reference: <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-ip-address-library>

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::compile_call_unary,
};

/// Compile a Kubernetes IP address extension function/method call.
pub fn compile_k8s_ip_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    // `ip.isCanonical(string)` has a special calling convention:
    // The CEL parser sees `ip` as the receiver (target) and `isCanonical` as the
    // method name, with the string as the single argument.  We must ignore the
    // namespace `ip` target and compile only the string argument.
    if func_name == "isCanonical" {
        match &call_expr.target {
            Some(_) => {
                // Namespace-qualified style: ip.isCanonical(str)
                if call_expr.args.len() != 1 {
                    anyhow::bail!("ip.isCanonical() expects 1 argument");
                }
                compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
            }
            None => {
                // Direct style (uncommon but handle gracefully): isCanonical(str)
                if call_expr.args.len() != 1 {
                    anyhow::bail!("isCanonical() expects 1 argument");
                }
                compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
            }
        }
        body.call(env.get(RuntimeFunction::K8sIpIsCanonical));
        return Ok(());
    }

    let runtime_fn = match func_name {
        "ip" => RuntimeFunction::K8sIpParse,
        "isIP" => RuntimeFunction::K8sIsIp,
        "family" => RuntimeFunction::K8sIpFamily,
        "isUnspecified" => RuntimeFunction::K8sIpIsUnspecified,
        "isLoopback" => RuntimeFunction::K8sIpIsLoopback,
        "isLinkLocalMulticast" => RuntimeFunction::K8sIpIsLinkLocalMulticast,
        "isLinkLocalUnicast" => RuntimeFunction::K8sIpIsLinkLocalUnicast,
        "isGlobalUnicast" => RuntimeFunction::K8sIpIsGlobalUnicast,
        _ => anyhow::bail!("Unknown Kubernetes IP function: {}", func_name),
    };

    compile_call_unary(call_expr, func_name, runtime_fn, body, env, ctx, module)
}
