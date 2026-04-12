pub mod conversions;
pub mod ext;
pub mod extensions;
pub mod kubernetes;
pub mod optional;
pub mod strings;
pub mod temporal;

use cel::common::ast::CallExpr;
use walrus::InstrSeqBuilder;

use crate::compiler::context::{CompilerContext, CompilerEnv};

/// Dispatch a named function call (non-operator) to the appropriate sub-module.
pub fn compile_named_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    // Try optional functions first (some names like "none", "of", "or", "value" etc.
    // are only treated as optional when target is the "optional" namespace or receiver is Optional)
    if optional::compile_optional_function(func_name, call_expr, body, env, ctx, module)? {
        return Ok(());
    }

    match func_name {
        // Core string functions
        "size" | "startsWith" | "endsWith" | "contains" | "matches" => {
            strings::compile_string_function(func_name, call_expr, body, env, ctx, module)
        }
        // Extended string library
        "lowerAscii" | "upperAscii" | "trim" | "reverse" | "charAt" | "replace" | "split"
        | "substring" | "format" | "quote" => {
            ext::strings::compile_ext_string_function(func_name, call_expr, body, env, ctx, module)
        }
        // Extended list library
        "join" => {
            ext::lists::compile_ext_list_function(func_name, call_expr, body, env, ctx, module)
        }
        // indexOf / lastIndexOf: overloaded by arity (polymorphic or string+offset)
        "indexOf" | "lastIndexOf" => {
            ext::poly::compile_index_of_dispatch(func_name, call_expr, body, env, ctx, module)
        }
        "timestamp" | "duration" | "getFullYear" | "getMonth" | "getDate" | "getDayOfMonth"
        | "getDayOfWeek" | "getDayOfYear" | "getHours" | "getMinutes" | "getSeconds"
        | "getMilliseconds" => {
            temporal::compile_temporal_function(func_name, call_expr, body, env, ctx, module)
        }
        "string" | "int" | "uint" | "double" | "bytes" | "bool" | "type" | "dyn" => {
            conversions::compile_conversion_function(func_name, call_expr, body, env, ctx, module)
        }
        "isSorted" | "sum" | "min" | "max" => kubernetes::lists::compile_k8s_list_function(
            func_name, call_expr, body, env, ctx, module,
        ),
        "find" | "findAll" => kubernetes::regex::compile_k8s_regex_function(
            func_name, call_expr, body, env, ctx, module,
        ),
        "url" | "isURL" | "getScheme" | "getHost" | "getHostname" | "getPort"
        | "getEscapedPath" | "getQuery" => {
            kubernetes::url::compile_k8s_url_function(func_name, call_expr, body, env, ctx, module)
        }
        // CIDR-specific methods (unambiguous names)
        "cidr" | "isCIDR" | "masked" | "prefixLength" | "containsIP" | "containsCIDR" => {
            kubernetes::cidr::compile_k8s_cidr_function(
                func_name, call_expr, body, env, ctx, module,
            )
        }
        // `ip()` method on a CIDR receiver: `cidr_value.ip()` → route to CIDR module.
        // `ip(string)` constructor (no target): route to IP module.
        "ip" if call_expr.target.is_some() && call_expr.args.is_empty() => {
            kubernetes::cidr::compile_k8s_cidr_function(
                func_name, call_expr, body, env, ctx, module,
            )
        }
        "ip"
        | "isIP"
        | "isCanonical"
        | "family"
        | "isUnspecified"
        | "isLoopback"
        | "isLinkLocalMulticast"
        | "isLinkLocalUnicast"
        | "isGlobalUnicast" => {
            kubernetes::ip::compile_k8s_ip_function(func_name, call_expr, body, env, ctx, module)
        }
        // Semver functions and methods (excluding shared comparison methods)
        "isSemver" | "semver" | "major" | "minor" | "patch" => {
            kubernetes::semver::compile_k8s_semver_function(
                func_name, call_expr, body, env, ctx, module,
            )
        }
        // Polymorphic comparison methods — shared between Semver and Quantity.
        // Routed through the quantity dispatcher which uses polymorphic runtime functions.
        "isLessThan" | "isGreaterThan" | "compareTo" => {
            kubernetes::quantity::compile_k8s_quantity_function(
                func_name, call_expr, body, env, ctx, module,
            )
        }
        // Quantity functions and methods
        "quantity" | "isQuantity" | "sign" | "isInteger" | "asInteger" | "asApproximateFloat"
        | "add" | "sub" => kubernetes::quantity::compile_k8s_quantity_function(
            func_name, call_expr, body, env, ctx, module,
        ),
        _ => extensions::compile_extension_call(call_expr, body, env, ctx, module),
    }
}
