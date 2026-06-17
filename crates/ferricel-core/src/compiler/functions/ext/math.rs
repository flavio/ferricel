//! Compiler for CEL math extension library functions.
//!
//! Handles: `math.greatest`, `math.least`, `math.ceil`, `math.floor`,
//! `math.round`, `math.trunc`, `math.abs`, `math.sign`, `math.isInf`,
//! `math.isNaN`, `math.isFinite`, `math.bitAnd`, `math.bitOr`, `math.bitXor`,
//! `math.bitNot`, `math.bitShiftLeft`, `math.bitShiftRight`, `math.sqrt`.
//!
//! All functions are namespace-qualified with the `math` prefix. The caller
//! in `functions/mod.rs` already guards on the `math` namespace before
//! dispatching here.
//!
//! **greatest / least variadic handling**
//!
//! `math.greatest` and `math.least` accept 1..N numeric arguments or a single
//! list argument. The runtime function (`cel_math_greatest` / `cel_math_least`)
//! always receives a single `CelValue`:
//! - 1 arg: passed directly
//! - N args: wrapped in a freshly-created `Array` at compile time and passed
//!   as a list (same approach as `compile_list` in `collections.rs`)

use cel::common::ast::{CallExpr, IdedExpr};
use ferricel_types::functions::RuntimeFunction;
use walrus::{InstrSeqBuilder, ValType};

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    operators::{compile_binary_op, compile_unary_op},
};

/// Dispatch to the correct math extension function compiler.
pub fn compile_ext_math_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        // Variadic min/max
        "greatest" => compile_math_minmax(
            call_expr,
            body,
            env,
            ctx,
            module,
            RuntimeFunction::MathGreatest,
        )?,
        "least" => compile_math_minmax(
            call_expr,
            body,
            env,
            ctx,
            module,
            RuntimeFunction::MathLeast,
        )?,

        // Unary double → double
        "ceil" => compile_unary_op(
            call_expr,
            "math.ceil()",
            RuntimeFunction::MathCeil,
            body,
            env,
            ctx,
            module,
        )?,
        "floor" => compile_unary_op(
            call_expr,
            "math.floor()",
            RuntimeFunction::MathFloor,
            body,
            env,
            ctx,
            module,
        )?,
        "round" => compile_unary_op(
            call_expr,
            "math.round()",
            RuntimeFunction::MathRound,
            body,
            env,
            ctx,
            module,
        )?,
        "trunc" => compile_unary_op(
            call_expr,
            "math.trunc()",
            RuntimeFunction::MathTrunc,
            body,
            env,
            ctx,
            module,
        )?,

        // Unary polymorphic
        "abs" => compile_unary_op(
            call_expr,
            "math.abs()",
            RuntimeFunction::MathAbs,
            body,
            env,
            ctx,
            module,
        )?,
        "sign" => compile_unary_op(
            call_expr,
            "math.sign()",
            RuntimeFunction::MathSign,
            body,
            env,
            ctx,
            module,
        )?,

        // Unary double → bool
        "isInf" => compile_unary_op(
            call_expr,
            "math.isInf()",
            RuntimeFunction::MathIsInf,
            body,
            env,
            ctx,
            module,
        )?,
        "isNaN" => compile_unary_op(
            call_expr,
            "math.isNaN()",
            RuntimeFunction::MathIsNaN,
            body,
            env,
            ctx,
            module,
        )?,
        "isFinite" => compile_unary_op(
            call_expr,
            "math.isFinite()",
            RuntimeFunction::MathIsFinite,
            body,
            env,
            ctx,
            module,
        )?,

        // Binary bitwise
        "bitOr" => compile_binary_op(
            call_expr,
            "math.bitOr()",
            RuntimeFunction::MathBitOr,
            body,
            env,
            ctx,
            module,
        )?,
        "bitAnd" => compile_binary_op(
            call_expr,
            "math.bitAnd()",
            RuntimeFunction::MathBitAnd,
            body,
            env,
            ctx,
            module,
        )?,
        "bitXor" => compile_binary_op(
            call_expr,
            "math.bitXor()",
            RuntimeFunction::MathBitXor,
            body,
            env,
            ctx,
            module,
        )?,

        // Unary bitwise
        "bitNot" => compile_unary_op(
            call_expr,
            "math.bitNot()",
            RuntimeFunction::MathBitNot,
            body,
            env,
            ctx,
            module,
        )?,

        // Binary shift
        "bitShiftLeft" => compile_binary_op(
            call_expr,
            "math.bitShiftLeft()",
            RuntimeFunction::MathBitShiftLeft,
            body,
            env,
            ctx,
            module,
        )?,
        "bitShiftRight" => compile_binary_op(
            call_expr,
            "math.bitShiftRight()",
            RuntimeFunction::MathBitShiftRight,
            body,
            env,
            ctx,
            module,
        )?,

        // Unary sqrt (int|uint|double → double)
        "sqrt" => compile_unary_op(
            call_expr,
            "math.sqrt()",
            RuntimeFunction::MathSqrt,
            body,
            env,
            ctx,
            module,
        )?,

        _ => anyhow::bail!("Unknown math extension function: math.{}", func_name),
    }
    Ok(())
}

/// Compile `math.greatest` / `math.least` with variadic argument support.
///
/// Accepted call shapes (from the CEL-Go spec):
/// - `math.greatest(x)` — single scalar OR single list variable/expression
/// - `math.greatest(x, y)` — two numeric args
/// - `math.greatest(x, y, z, ...)` — three or more numeric args
/// - `math.greatest([x, y, z])` — a list literal (parsed by the CEL parser as
///   a single arg that is an `Expr::List`)
///
/// For the single-argument case the value is passed directly.
/// For the multi-argument case the args are wrapped in a fresh `Array` at
/// compile time (the same way `compile_list` works in `collections.rs`).
fn compile_math_minmax(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
    runtime_fn: RuntimeFunction,
) -> Result<(), anyhow::Error> {
    if call_expr.args.is_empty() {
        anyhow::bail!("math.{}() requires at least one argument", runtime_fn);
    }

    if call_expr.args.len() == 1 {
        // Single argument: could be a scalar, a variable, or a list expression.
        // Pass it through as-is; the runtime handles both scalars and arrays.
        compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    } else {
        // Multiple arguments: wrap them in a temporary array so the runtime
        // always receives a `CelValue::Array`.
        compile_args_as_array(&call_expr.args, body, env, ctx, module)?;
    }

    body.call(env.get(runtime_fn));
    Ok(())
}

/// Build a `CelValue::Array` on the Wasm stack containing the compiled values
/// of each element in `args`.  Equivalent to compiling `[args[0], args[1], ...]`.
fn compile_args_as_array(
    args: &[IdedExpr],
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    // Create empty array and stash its pointer in a local.
    // `cel_array_push` mutates in-place (returns void), so the original
    // pointer remains valid throughout — same pattern as `compile_list`.
    body.call(env.get(RuntimeFunction::CreateArray));
    let array_local = module.locals.add(ValType::I32);
    body.local_set(array_local);

    for arg in args {
        // Compile the element first (pushes element pointer onto stack).
        compile_expr(&arg.expr, body, env, ctx, module)?;
        let elem_local = module.locals.add(ValType::I32);
        body.local_set(elem_local);

        // Load array pointer, then element, then call push (void return).
        body.local_get(array_local);
        body.local_get(elem_local);
        body.call(env.get(RuntimeFunction::ArrayPush));
    }

    // Leave the array pointer on the stack.
    body.local_get(array_local);
    Ok(())
}
