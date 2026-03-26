pub mod conversions;
pub mod extensions;
pub mod kubernetes;
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
    match func_name {
        "size" | "startsWith" | "endsWith" | "contains" | "matches" => {
            strings::compile_string_function(func_name, call_expr, body, env, ctx, module)
        }
        "timestamp" | "duration" | "getFullYear" | "getMonth" | "getDate" | "getDayOfMonth"
        | "getDayOfWeek" | "getDayOfYear" | "getHours" | "getMinutes" | "getSeconds"
        | "getMilliseconds" => {
            temporal::compile_temporal_function(func_name, call_expr, body, env, ctx, module)
        }
        "string" | "int" | "uint" | "double" | "bytes" | "bool" | "type" | "dyn" => {
            conversions::compile_conversion_function(func_name, call_expr, body, env, ctx, module)
        }
        "isSorted" | "sum" | "min" | "max" | "indexOf" | "lastIndexOf" => {
            kubernetes::lists::compile_k8s_list_function(
                func_name, call_expr, body, env, ctx, module,
            )
        }
        "find" | "findAll" => kubernetes::regex::compile_k8s_regex_function(
            func_name, call_expr, body, env, ctx, module,
        ),
        _ => extensions::compile_extension_call(call_expr, body, env, ctx, module),
    }
}
