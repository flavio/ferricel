//! Compiler support for CEL optional types.
//!
//! Dispatches optional functions/operators to their runtime counterparts:
//!   - `optional.none()`              → `cel_optional_none`              (0-arg constructor)
//!   - `optional.of(x)`               → `cel_optional_of`                (unary constructor)
//!   - `optional.ofNonZeroValue(x)`   → `cel_optional_of_non_zero_value` (unary constructor)
//!   - `<opt>.hasValue()`             → `cel_optional_has_value`         (unary method)
//!   - `<opt>.value()`                → `cel_optional_value`             (unary method)
//!   - `<opt>.orValue(default)`       → `cel_optional_or_value`          (binary method)
//!   - `<opt>.or(other_opt)`          → `cel_optional_or`                (binary method)
//!   - `<opt>.optMap(var, body)`      → inline WASM: if hasValue then of(body[var=value]) else none
//!   - `<opt>.optFlatMap(var, body)`  → inline WASM: if hasValue then body[var=value] else none
//!
//! Reference: CEL spec optional types extension.

use cel::common::ast::{CallExpr, Expr};
use ferricel_types::functions::RuntimeFunction;
use walrus::{InstrSeqBuilder, ValType};

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::{compile_call_binary, compile_call_unary},
};

/// Compile an `optional.none()` call.
///
/// CEL form: `optional.none()`
/// AST: `func_name="none"`, `target=Some(Ident("optional"))`, `args=[]`
fn compile_optional_none(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
) -> Result<(), anyhow::Error> {
    if !call_expr.args.is_empty() {
        anyhow::bail!("optional.none() expects 0 arguments");
    }
    body.call(env.get(RuntimeFunction::OptionalNone));
    Ok(())
}

/// Compile `optional.of(x)` — wraps x unconditionally.
///
/// CEL form: `optional.of(x)`
/// AST: `func_name="of"`, `target=Some(Ident("optional"))`, `args=[x]`
fn compile_optional_of(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 1 {
        anyhow::bail!("optional.of() expects 1 argument");
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    body.call(env.get(RuntimeFunction::OptionalOf));
    Ok(())
}

/// Compile `optional.ofNonZeroValue(x)` — wraps x if non-zero, else none.
///
/// CEL form: `optional.ofNonZeroValue(x)`
/// AST: `func_name="ofNonZeroValue"`, `target=Some(Ident("optional"))`, `args=[x]`
fn compile_optional_of_non_zero_value(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 1 {
        anyhow::bail!("optional.ofNonZeroValue() expects 1 argument");
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    body.call(env.get(RuntimeFunction::OptionalOfNonZeroValue));
    Ok(())
}

/// Compile `<opt>.optMap(var, body)`.
///
/// CEL form: `opt.optMap(var, body_expr)`
/// AST: `func_name="optMap"`, `target=Some(opt_expr)`, `args=[ident_var, body_expr]`
///
/// Semantics: if opt has a value, bind the inner value to `var` and evaluate
/// `body_expr`, wrap the result in `optional.of(result)`. If opt is none, return none.
///
/// Generated WASM logic:
/// ```
/// opt_val = compile(target)
/// if cel_optional_has_value(opt_val) == Bool(true):
///     inner = cel_optional_value(opt_val)
///     result = compile(body_expr) with var=inner
///     return cel_optional_of(result)
/// else:
///     return cel_optional_none()
/// ```
fn compile_opt_map(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let target = call_expr
        .target
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("optMap requires a target (receiver)"))?;

    if call_expr.args.len() != 2 {
        anyhow::bail!("optMap() expects 2 arguments (var, body)");
    }

    // Extract the variable name from args[0] (must be an Ident)
    let var_name = match &call_expr.args[0].expr {
        Expr::Ident(name) => name.clone(),
        _ => anyhow::bail!("optMap first argument must be an identifier"),
    };

    // Compile the target optional expression
    compile_expr(&target.expr, body, env, ctx, module)?;
    let opt_local = module.locals.add(ValType::I32);
    body.local_tee(opt_local);

    // Check hasValue: cel_optional_has_value(opt) → CelValue::Bool
    body.call(env.get(RuntimeFunction::OptionalHasValue));
    // Convert CelValue::Bool to i64 via cel_value_to_bool, then wrap to i32
    body.call(env.get(RuntimeFunction::ValueToBool));
    body.unop(walrus::ir::UnaryOp::I32WrapI64);

    // Branch: if hasValue → extract, map, re-wrap; else → none
    let then_seq = body.dangling_instr_seq(Some(ValType::I32));
    let then_id = then_seq.id();
    let else_seq = body.dangling_instr_seq(Some(ValType::I32));
    let else_id = else_seq.id();

    body.instr(walrus::ir::IfElse {
        consequent: then_id,
        alternative: else_id,
    });

    // Then branch: extract inner value, bind to var, compile body, wrap in Optional
    {
        let mut then_body = body.instr_seq(then_id);
        then_body.local_get(opt_local);
        then_body.call(env.get(RuntimeFunction::OptionalValue));
        let inner_local = module.locals.add(ValType::I32);
        then_body.local_set(inner_local);

        let inner_ctx = ctx.with_local(var_name, inner_local);
        compile_expr(
            &call_expr.args[1].expr,
            &mut then_body,
            env,
            &inner_ctx,
            module,
        )?;
        then_body.call(env.get(RuntimeFunction::OptionalOf));
    }

    // Else branch: return none
    {
        body.instr_seq(else_id)
            .call(env.get(RuntimeFunction::OptionalNone));
    }

    Ok(())
}

/// Compile `<opt>.optFlatMap(var, body)`.
///
/// Same as `optMap` but the body expression already returns an `Optional`,
/// so we do NOT wrap it again in `optional.of`.
fn compile_opt_flat_map(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let target = call_expr
        .target
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("optFlatMap requires a target (receiver)"))?;

    if call_expr.args.len() != 2 {
        anyhow::bail!("optFlatMap() expects 2 arguments (var, body)");
    }

    let var_name = match &call_expr.args[0].expr {
        Expr::Ident(name) => name.clone(),
        _ => anyhow::bail!("optFlatMap first argument must be an identifier"),
    };

    // Compile the target optional expression
    compile_expr(&target.expr, body, env, ctx, module)?;
    let opt_local = module.locals.add(ValType::I32);
    body.local_tee(opt_local);

    // Check hasValue
    body.call(env.get(RuntimeFunction::OptionalHasValue));
    body.call(env.get(RuntimeFunction::ValueToBool));
    body.unop(walrus::ir::UnaryOp::I32WrapI64);

    let then_seq = body.dangling_instr_seq(Some(ValType::I32));
    let then_id = then_seq.id();
    let else_seq = body.dangling_instr_seq(Some(ValType::I32));
    let else_id = else_seq.id();

    body.instr(walrus::ir::IfElse {
        consequent: then_id,
        alternative: else_id,
    });

    // Then branch: extract inner value, bind var, compile body (already Optional)
    {
        let mut then_body = body.instr_seq(then_id);
        then_body.local_get(opt_local);
        then_body.call(env.get(RuntimeFunction::OptionalValue));
        let inner_local = module.locals.add(ValType::I32);
        then_body.local_set(inner_local);

        let inner_ctx = ctx.with_local(var_name, inner_local);
        compile_expr(
            &call_expr.args[1].expr,
            &mut then_body,
            env,
            &inner_ctx,
            module,
        )?;
        // Do NOT wrap — body already returns Optional
    }

    // Else branch: return none
    {
        body.instr_seq(else_id)
            .call(env.get(RuntimeFunction::OptionalNone));
    }

    Ok(())
}

/// Returns true if the call expression targets the `optional` namespace.
///
/// Detects calls of the form `optional.none()`, `optional.of(x)`, `optional.ofNonZeroValue(x)`.
fn is_optional_namespace_call(call_expr: &CallExpr) -> bool {
    matches!(
        &call_expr.target,
        Some(t) if matches!(&t.expr, Expr::Ident(name) if name == "optional")
    )
}

/// Top-level dispatch for optional functions and methods.
///
/// Returns `Ok(true)` if the call was handled, `Ok(false)` otherwise.
pub fn compile_optional_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<bool, anyhow::Error> {
    match func_name {
        // Namespace-qualified constructors: optional.none() / optional.of(x) / optional.ofNonZeroValue(x)
        "none" if is_optional_namespace_call(call_expr) => {
            compile_optional_none(call_expr, body, env)?;
        }
        "of" if is_optional_namespace_call(call_expr) => {
            compile_optional_of(call_expr, body, env, ctx, module)?;
        }
        "ofNonZeroValue" if is_optional_namespace_call(call_expr) => {
            compile_optional_of_non_zero_value(call_expr, body, env, ctx, module)?;
        }

        // Methods on optional values
        "hasValue" => {
            compile_call_unary(
                call_expr,
                func_name,
                RuntimeFunction::OptionalHasValue,
                body,
                env,
                ctx,
                module,
            )?;
        }
        "value" => {
            compile_call_unary(
                call_expr,
                func_name,
                RuntimeFunction::OptionalValue,
                body,
                env,
                ctx,
                module,
            )?;
        }
        "orValue" => {
            compile_call_binary(
                call_expr,
                func_name,
                RuntimeFunction::OptionalOrValue,
                body,
                env,
                ctx,
                module,
            )?;
        }
        "or" if call_expr.target.is_some() => {
            // `<opt>.or(other)` — binary method on an optional receiver
            compile_call_binary(
                call_expr,
                func_name,
                RuntimeFunction::OptionalOr,
                body,
                env,
                ctx,
                module,
            )?;
        }

        // optMap / optFlatMap are macro-like — inline WASM with branching
        "optMap" => {
            compile_opt_map(call_expr, body, env, ctx, module)?;
        }
        "optFlatMap" => {
            compile_opt_flat_map(call_expr, body, env, ctx, module)?;
        }

        _ => return Ok(false),
    }

    Ok(true)
}
