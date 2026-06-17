//! Compiler support for Kubernetes CEL URL library extensions.
//!
//! Dispatches the Kubernetes URL functions to their runtime counterparts:
//!   - `url(string)`          → `cel_k8s_url_parse`          (unary, function-style)
//!   - `isURL(string)`        → `cel_k8s_is_url`             (unary, function-style)
//!   - `<URL>.getScheme()`    → `cel_k8s_url_get_scheme`     (unary, method-style)
//!   - `<URL>.getHost()`      → `cel_k8s_url_get_host`       (unary, method-style)
//!   - `<URL>.getHostname()`  → `cel_k8s_url_get_hostname`   (unary, method-style)
//!   - `<URL>.getPort()`      → `cel_k8s_url_get_port`       (unary, method-style)
//!   - `<URL>.getEscapedPath()` → `cel_k8s_url_get_escaped_path` (unary, method-style)
//!   - `<URL>.getQuery()`     → `cel_k8s_url_get_query`      (unary, method-style)
//!
//! Reference: <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-url-library>

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    helpers::compile_call_unary,
};

/// Compile a Kubernetes URL extension function/method call.
pub fn compile_k8s_url_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let runtime_fn = match func_name {
        "url" => RuntimeFunction::K8sUrlParse,
        "isURL" => RuntimeFunction::K8sIsUrl,
        "getScheme" => RuntimeFunction::K8sUrlGetScheme,
        "getHost" => RuntimeFunction::K8sUrlGetHost,
        "getHostname" => RuntimeFunction::K8sUrlGetHostname,
        "getPort" => RuntimeFunction::K8sUrlGetPort,
        "getEscapedPath" => RuntimeFunction::K8sUrlGetEscapedPath,
        "getQuery" => RuntimeFunction::K8sUrlGetQuery,
        _ => anyhow::bail!("Unknown Kubernetes URL function: {}", func_name),
    };

    compile_call_unary(call_expr, func_name, runtime_fn, body, env, ctx, module)
}
