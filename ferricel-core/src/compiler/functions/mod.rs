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
        // cel.bind(var, init, body) — variable binding macro.
        // Must come before any unguarded arm to avoid shadowing user-defined "bind" functions.
        "bind"
            if matches!(
                &call_expr.target,
                Some(t) if matches!(&t.expr, cel::common::ast::Expr::Ident(name) if name == "cel")
            ) =>
        {
            ext::bind::compile_cel_bind(call_expr, body, env, ctx, module)
        }
        // cel.block(bindings, body) — common subexpression elimination construct.
        "block"
            if matches!(
                &call_expr.target,
                Some(t) if matches!(&t.expr, cel::common::ast::Expr::Ident(name) if name == "cel")
            ) =>
        {
            ext::block::compile_cel_block(call_expr, body, env, ctx, module)
        }
        // cel.index(N) — reference slot N of the enclosing cel.block.
        "index"
            if matches!(
                &call_expr.target,
                Some(t) if matches!(&t.expr, cel::common::ast::Expr::Ident(name) if name == "cel")
            ) =>
        {
            ext::block::compile_cel_index(call_expr, body, env, ctx, module)
        }
        // cel.iterVar(N, M) — reference comprehension iteration variable at depth N, scope M.
        "iterVar"
            if matches!(
                &call_expr.target,
                Some(t) if matches!(&t.expr, cel::common::ast::Expr::Ident(name) if name == "cel")
            ) =>
        {
            ext::block::compile_cel_iter_var(call_expr, body, env, ctx, module)
        }
        // cel.accuVar(N, M) — reference comprehension accumulator variable at depth N, scope M.
        "accuVar"
            if matches!(
                &call_expr.target,
                Some(t) if matches!(&t.expr, cel::common::ast::Expr::Ident(name) if name == "cel")
            ) =>
        {
            ext::block::compile_cel_accu_var(call_expr, body, env, ctx, module)
        }
        // Two-variable comprehension macros — exists/all/existsOne with 3 args.
        // The CEL parser (v0.13.0) does NOT expand these as Comprehension nodes, so they
        // arrive here as regular Call nodes. Must come BEFORE the unguarded "exists" etc.
        // arms (there are none currently, but guard ensures correct routing if added later).
        "exists" | "all" | "existsOne" | "exists_one"
            if call_expr.args.len() == 3 && call_expr.target.is_some() =>
        {
            ext::comprehensions::compile_two_var_comprehension(
                func_name, call_expr, body, env, ctx, module,
            )
        }
        // transformList / transformMap / transformMapEntry — always two-variable (3 or 4 args).
        "transformList" | "transformMap" | "transformMapEntry"
            if (call_expr.args.len() == 3 || call_expr.args.len() == 4)
                && call_expr.target.is_some() =>
        {
            ext::comprehensions::compile_two_var_comprehension(
                func_name, call_expr, body, env, ctx, module,
            )
        }
        // Sets extension: sets.contains / sets.intersects / sets.equivalent
        // Must come BEFORE the core string arm that unconditionally matches "contains".
        // The guard on the "sets" namespace routes `sets.contains(...)` here instead of
        // to the string dispatcher.
        "contains" | "intersects" | "equivalent"
            if matches!(
                &call_expr.target,
                Some(t) if matches!(&t.expr, cel::common::ast::Expr::Ident(name) if name == "sets")
            ) =>
        {
            ext::sets::compile_ext_sets_function(func_name, call_expr, body, env, ctx, module)
        }
        // Core string functions
        "size" | "startsWith" | "endsWith" | "contains" | "matches" => {
            strings::compile_string_function(func_name, call_expr, body, env, ctx, module)
        }
        // Regex extension: regex.replace / regex.extract / regex.extractAll
        // This guard must come BEFORE the extended string library arm that also matches "replace",
        // so that `regex.replace(...)` is routed here instead of to the string dispatcher.
        "replace" | "extract" | "extractAll"
            if matches!(
                &call_expr.target,
                Some(t) if matches!(&t.expr, cel::common::ast::Expr::Ident(name) if name == "regex")
            ) =>
        {
            ext::regex::compile_ext_regex_function(func_name, call_expr, body, env, ctx, module)
        }
        // Extended string library.
        // Note: "reverse" is intentionally excluded here — it is polymorphic
        // (shared between strings and lists) and dispatched via ReversePoly below.
        "lowerAscii" | "upperAscii" | "trim" | "charAt" | "replace" | "split" | "substring"
        | "format" | "quote" => {
            ext::strings::compile_ext_string_function(func_name, call_expr, body, env, ctx, module)
        }
        // Polymorphic reverse: dispatches to cel_reverse_poly which handles both
        // String (character reversal) and Array (element reversal) receivers.
        "reverse" => {
            use crate::compiler::helpers::compile_call_unary;
            compile_call_unary(
                call_expr,
                "reverse",
                ferricel_types::functions::RuntimeFunction::ReversePoly,
                body,
                env,
                ctx,
                module,
            )
        }
        // lists.range(n) — namespace-qualified, no receiver.
        "range"
            if matches!(
                &call_expr.target,
                Some(t) if matches!(&t.expr, cel::common::ast::Expr::Ident(name) if name == "lists")
            ) =>
        {
            ext::lists::compile_list_range(call_expr, body, env, ctx, module)
        }
        // Encoders extension: base64.encode / base64.decode
        // Guard on the "base64" namespace to avoid collisions with user-defined functions.
        "encode" | "decode"
            if matches!(
                &call_expr.target,
                Some(t) if matches!(&t.expr, cel::common::ast::Expr::Ident(name) if name == "base64")
            ) =>
        {
            ext::encoders::compile_ext_encoder_function(
                func_name, call_expr, body, env, ctx, module,
            )
        }
        // Math extension: math.greatest, math.least, math.ceil, math.floor, math.round,
        // math.trunc, math.abs, math.sign, math.isInf, math.isNaN, math.isFinite,
        // math.bitAnd, math.bitOr, math.bitXor, math.bitNot, math.bitShiftLeft,
        // math.bitShiftRight, math.sqrt
        "greatest" | "least" | "ceil" | "floor" | "round" | "trunc" | "abs" | "sign" | "isInf"
        | "isNaN" | "isFinite" | "bitOr" | "bitAnd" | "bitXor" | "bitNot" | "bitShiftLeft"
        | "bitShiftRight" | "sqrt"
            if matches!(
                &call_expr.target,
                Some(t) if matches!(&t.expr, cel::common::ast::Expr::Ident(name) if name == "math")
            ) =>
        {
            ext::math::compile_ext_math_function(func_name, call_expr, body, env, ctx, module)
        }
        // Extended list library
        "join" | "distinct" | "flatten" | "slice" | "sort" | "sortBy" | "first" | "last" => {
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
        // Kubernetes Format library: format.named() and format.<name>() constructors, <fmt>.validate()
        "named"
        | "validate"
        | "dns1123Label"
        | "dns1123Subdomain"
        | "dns1035Label"
        | "qualifiedName"
        | "dns1123LabelPrefix"
        | "dns1123SubdomainPrefix"
        | "dns1035LabelPrefix"
        | "labelValue"
        | "uri"
        | "uuid"
        | "byte"
        | "date"
        | "datetime" => kubernetes::format::compile_k8s_format_function(
            func_name, call_expr, body, env, ctx, module,
        ),
        _ => extensions::compile_extension_call(call_expr, body, env, ctx, module),
    }
}
