//! Compiler support for Kubernetes CEL Format library extensions.
//!
//! Dispatches the Kubernetes format functions to their runtime counterparts:
//!
//!   - `format.named(string)`           → `cel_k8s_format_named`              (namespace unary)
//!   - `format.dns1123Label()`          → `cel_k8s_format_dns1123_label`       (namespace 0-arg)
//!   - `format.dns1123Subdomain()`      → `cel_k8s_format_dns1123_subdomain`   (namespace 0-arg)
//!   - `format.dns1035Label()`          → `cel_k8s_format_dns1035_label`       (namespace 0-arg)
//!   - `format.qualifiedName()`         → `cel_k8s_format_qualified_name`      (namespace 0-arg)
//!   - `format.dns1123LabelPrefix()`    → `cel_k8s_format_dns1123_label_prefix`    (namespace 0-arg)
//!   - `format.dns1123SubdomainPrefix()`→ `cel_k8s_format_dns1123_subdomain_prefix`(namespace 0-arg)
//!   - `format.dns1035LabelPrefix()`    → `cel_k8s_format_dns1035_label_prefix`(namespace 0-arg)
//!   - `format.labelValue()`            → `cel_k8s_format_label_value`         (namespace 0-arg)
//!   - `format.uri()`                   → `cel_k8s_format_uri`                 (namespace 0-arg)
//!   - `format.uuid()`                  → `cel_k8s_format_uuid`                (namespace 0-arg)
//!   - `format.byte()`                  → `cel_k8s_format_byte`                (namespace 0-arg)
//!   - `format.date()`                  → `cel_k8s_format_date`                (namespace 0-arg)
//!   - `format.datetime()`              → `cel_k8s_format_datetime`            (namespace 0-arg)
//!   - `<Format>.validate(string)`      → `cel_k8s_format_validate`            (binary method)
//!
//! Reference: <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-format-library>

use cel::common::ast::{CallExpr, Expr};
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::compile_call_binary,
};

/// Returns `true` if the call expression targets the `format` namespace.
///
/// Detects: `format.named(x)`, `format.dns1123Label()`, etc.
fn is_format_namespace_call(call_expr: &CallExpr) -> bool {
    matches!(
        &call_expr.target,
        Some(t) if matches!(&t.expr, Expr::Ident(name) if name == "format")
    )
}

/// Compile a Kubernetes Format extension function/method call.
pub fn compile_k8s_format_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    // Namespace-qualified zero-arg constructors: format.<name>()
    if is_format_namespace_call(call_expr) {
        let runtime_fn = match func_name {
            "named" => {
                // format.named(string) → unary
                if call_expr.args.len() != 1 {
                    anyhow::bail!("format.named() expects 1 argument");
                }
                compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
                body.call(env.get(RuntimeFunction::K8sFormatNamed));
                return Ok(());
            }
            "dns1123Label" => RuntimeFunction::K8sFormatDns1123Label,
            "dns1123Subdomain" => RuntimeFunction::K8sFormatDns1123Subdomain,
            "dns1035Label" => RuntimeFunction::K8sFormatDns1035Label,
            "qualifiedName" => RuntimeFunction::K8sFormatQualifiedName,
            "dns1123LabelPrefix" => RuntimeFunction::K8sFormatDns1123LabelPrefix,
            "dns1123SubdomainPrefix" => RuntimeFunction::K8sFormatDns1123SubdomainPrefix,
            "dns1035LabelPrefix" => RuntimeFunction::K8sFormatDns1035LabelPrefix,
            "labelValue" => RuntimeFunction::K8sFormatLabelValue,
            "uri" => RuntimeFunction::K8sFormatUri,
            "uuid" => RuntimeFunction::K8sFormatUuid,
            "byte" => RuntimeFunction::K8sFormatByte,
            "date" => RuntimeFunction::K8sFormatDate,
            "datetime" => RuntimeFunction::K8sFormatDatetime,
            _ => anyhow::bail!("Unknown format constructor: format.{}()", func_name),
        };
        if !call_expr.args.is_empty() {
            anyhow::bail!("format.{}() takes no arguments", func_name);
        }
        body.call(env.get(runtime_fn));
        return Ok(());
    }

    // Method on Format value: <fmt>.validate(string)
    if func_name == "validate" {
        return compile_call_binary(
            call_expr,
            "validate",
            RuntimeFunction::K8sFormatValidate,
            body,
            env,
            ctx,
            module,
        );
    }

    anyhow::bail!("Unknown Kubernetes format function: {}", func_name)
}
