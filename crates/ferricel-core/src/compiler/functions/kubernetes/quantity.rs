//! Compiler support for Kubernetes CEL quantity library extensions.
//!
//! Dispatches the Kubernetes quantity functions to their runtime counterparts:
//!   - `quantity(string)`                   → `cel_k8s_quantity_parse`
//!   - `isQuantity(string)`                 → `cel_k8s_is_quantity`
//!   - `<Q>.sign()`                         → `cel_k8s_quantity_sign`
//!   - `<Q>.isInteger()`                    → `cel_k8s_quantity_is_integer`
//!   - `<Q>.asInteger()`                    → `cel_k8s_quantity_as_integer`
//!   - `<Q>.asApproximateFloat()`           → `cel_k8s_quantity_as_approx_float`
//!   - `<Q>.add(<Q>)`                       → `cel_k8s_quantity_add`
//!   - `<Q>.add(int)`                       → `cel_k8s_quantity_add_int`
//!   - `<Q>.sub(<Q>)`                       → `cel_k8s_quantity_sub`
//!   - `<Q>.sub(int)`                       → `cel_k8s_quantity_sub_int`
//!   - `<Q>.isLessThan(<Q>)`               → `cel_k8s_poly_is_less_than`
//!   - `<Q>.isGreaterThan(<Q>)`            → `cel_k8s_poly_is_greater_than`
//!   - `<Q>.compareTo(<Q>)`                → `cel_k8s_poly_compare_to`
//!
//! ## isLessThan / isGreaterThan / compareTo
//!
//! These three method names are shared with the semver library. Because the
//! compiler has no type information at the call site, all three are routed to
//! the runtime dispatch functions in `runtime/src/kubernetes/dispatch.rs`, which
//! inspect the receiver type at runtime and forward to the right implementation.
//!
//! Reference: <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-quantity-library>

use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    helpers::{compile_call_binary, compile_call_unary},
};

/// Compile a Kubernetes quantity extension function/method call.
pub fn compile_k8s_quantity_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        // quantity(string) — constructor
        "quantity" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sQuantityParse,
            body,
            env,
            ctx,
            module,
        ),

        // isQuantity(string) — validator
        "isQuantity" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sIsQuantity,
            body,
            env,
            ctx,
            module,
        ),

        // Unary method-style accessors
        "sign" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sQuantitySign,
            body,
            env,
            ctx,
            module,
        ),
        "isInteger" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sQuantityIsInteger,
            body,
            env,
            ctx,
            module,
        ),
        "asInteger" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sQuantityAsInteger,
            body,
            env,
            ctx,
            module,
        ),
        "asApproximateFloat" => compile_call_unary(
            call_expr,
            func_name,
            RuntimeFunction::K8sQuantityAsApproxFloat,
            body,
            env,
            ctx,
            module,
        ),

        // Binary method-style: add/sub are overloaded (Quantity or Int second arg)
        // We check the number of arguments and the type of the second argument.
        // Since we can't know the type at compile time, we check the AST expression kind.
        "add" => {
            // Determine which variant based on the second argument expression
            let second_arg = if let Some(_target) = &call_expr.target {
                // method style: target.add(arg)
                if call_expr.args.len() != 1 {
                    anyhow::bail!("add() method expects 1 argument");
                }
                &call_expr.args[0].expr
            } else {
                // function style: add(receiver, arg)
                if call_expr.args.len() != 2 {
                    anyhow::bail!("add() function expects 2 arguments");
                }
                &call_expr.args[1].expr
            };

            // Check if the second argument is an integer literal or int-typed expression
            if is_int_expr(second_arg) {
                compile_call_binary(
                    call_expr,
                    func_name,
                    RuntimeFunction::K8sQuantityAddInt,
                    body,
                    env,
                    ctx,
                    module,
                )
            } else {
                compile_call_binary(
                    call_expr,
                    func_name,
                    RuntimeFunction::K8sQuantityAdd,
                    body,
                    env,
                    ctx,
                    module,
                )
            }
        }

        "sub" => {
            let second_arg = if let Some(_target) = &call_expr.target {
                if call_expr.args.len() != 1 {
                    anyhow::bail!("sub() method expects 1 argument");
                }
                &call_expr.args[0].expr
            } else {
                if call_expr.args.len() != 2 {
                    anyhow::bail!("sub() function expects 2 arguments");
                }
                &call_expr.args[1].expr
            };

            if is_int_expr(second_arg) {
                compile_call_binary(
                    call_expr,
                    func_name,
                    RuntimeFunction::K8sQuantitySubInt,
                    body,
                    env,
                    ctx,
                    module,
                )
            } else {
                compile_call_binary(
                    call_expr,
                    func_name,
                    RuntimeFunction::K8sQuantitySub,
                    body,
                    env,
                    ctx,
                    module,
                )
            }
        }

        // Binary comparison methods — polymorphic: dispatch based on receiver type at runtime
        "isLessThan" => compile_call_binary(
            call_expr,
            func_name,
            RuntimeFunction::K8sPolyIsLessThan,
            body,
            env,
            ctx,
            module,
        ),
        "isGreaterThan" => compile_call_binary(
            call_expr,
            func_name,
            RuntimeFunction::K8sPolyIsGreaterThan,
            body,
            env,
            ctx,
            module,
        ),
        "compareTo" => compile_call_binary(
            call_expr,
            func_name,
            RuntimeFunction::K8sPolyCompareTo,
            body,
            env,
            ctx,
            module,
        ),

        _ => anyhow::bail!("Unknown Kubernetes quantity function: {}", func_name),
    }
}

/// Heuristic: returns true if the expression looks like an integer (literal or
/// negated integer literal). This is used to select the correct `add`/`sub`
/// overload at compile time.
fn is_int_expr(expr: &cel::common::ast::Expr) -> bool {
    use cel::common::ast::{Expr, LiteralValue};
    match expr {
        Expr::Literal(LiteralValue::Int(_)) => true,
        Expr::Literal(LiteralValue::UInt(_)) => true,
        // Negated int literal: -N appears as a unary minus call
        Expr::Call(call) if call.func_name == "_-_" || call.func_name == "-_" => {
            call.args.len() == 1 && {
                matches!(&call.args[0].expr, Expr::Literal(LiteralValue::Int(_)))
            }
        }
        _ => false,
    }
}
