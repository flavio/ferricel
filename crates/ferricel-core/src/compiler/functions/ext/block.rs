//! Compiler support for `cel.block`, `cel.index`, `cel.iterVar`, and `cel.accuVar`.
//!
//! ## `cel.block(bindings, body)`
//!
//! A common-subexpression-elimination (CSE) construct. It evaluates a list of
//! slot expressions in order, binding each to a numbered local (`@index0`,
//! `@index1`, …). Later slots may reference earlier ones. The body expression
//! is then evaluated in the context where all slots are available.
//!
//! ```text
//! cel.block([1, cel.index(0) + 1, cel.index(1) + 1], cel.index(2))
//!   → @index0 = 1
//!   → @index1 = @index0 + 1  =  2
//!   → @index2 = @index1 + 1  =  3
//!   → result  = @index2      =  3
//! ```
//!
//! ## `cel.index(N)` (test-only macro)
//!
//! References slot N set up by the enclosing `cel.block`. Compiles to a local
//! variable lookup for `@indexN`.
//!
//! ## `cel.iterVar(N, M)` / `cel.accuVar(N, M)` (test-only macros)
//!
//! Reference comprehension iteration / accumulator variables by nesting depth N
//! and scope M. Compile to identifier lookups for `@it:N:M` / `@ac:N:M`
//! respectively.
//!
//! **Note:** `cel.iterVar` and `cel.accuVar` cannot currently be used as the
//! iteration variable argument in comprehension macros (`map`, `filter`, etc.)
//! because the upstream `cel` crate parser requires a plain `Ident` there.
//! They work correctly when used *inside* the comprehension body.

use cel::common::ast::{CallExpr, Expr, LiteralValue};
use walrus::{InstrSeqBuilder, ValType};

use crate::compiler::{
    access::compile_ident,
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
};

/// Compile `cel.block(bindings_list, body)`.
///
/// - `args[0]` must be a `List` expression (the slot bindings).
/// - `args[1]` is the body expression evaluated with all slots in scope.
pub fn compile_cel_block(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 2 {
        anyhow::bail!(
            "cel.block() expects exactly 2 arguments (bindings, body), got {}",
            call_expr.args.len()
        );
    }

    let bindings_expr = &call_expr.args[0].expr;
    let body_expr = &call_expr.args[1].expr;

    let slots = match bindings_expr {
        Expr::List(list) => &list.elements,
        _ => anyhow::bail!("cel.block() first argument must be a list literal"),
    };

    // Compile each slot in order, storing results in fresh Wasm locals.
    // Each slot is added to the context as @indexN before the next slot is compiled,
    // so later slots can reference earlier ones.
    //
    // We build a chain of child contexts: ctx → ctx+@index0 → ctx+@index0+@index1 → …
    // We use an Option<CompilerContext> to avoid needing Clone on the initial ctx.
    let mut current_ctx;
    let mut ctx_ref: &CompilerContext = ctx;

    // We need owned contexts for the chain; we'll hold them in a Vec so they stay alive.
    let mut owned_ctxs: Vec<CompilerContext> = Vec::with_capacity(slots.len());

    for (i, slot_expr) in slots.iter().enumerate() {
        compile_expr(&slot_expr.expr, body, env, ctx_ref, module)?;
        let local = module.locals.add(ValType::I32);
        body.local_set(local);
        current_ctx = ctx_ref.with_local(format!("@index{}", i), local);
        owned_ctxs.push(current_ctx);
        ctx_ref = owned_ctxs.last().unwrap();
    }

    // Compile the body in the fully-populated slot context.
    compile_expr(body_expr, body, env, ctx_ref, module)
}

/// Compile `cel.index(N)` — reference slot N of the enclosing `cel.block`.
///
/// Rewrites to a lookup of the local variable `@indexN`.
pub fn compile_cel_index(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let n = extract_non_negative_int_arg(call_expr, "cel.index", 0)?;
    let var_name = format!("@index{}", n);
    compile_ident(&var_name, body, env, ctx, module)
}

/// Compile `cel.iterVar(N, M)` — reference the iteration variable at
/// comprehension nesting depth N, scope M.
///
/// Rewrites to a lookup of `@it:N:M`.
pub fn compile_cel_iter_var(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 2 {
        anyhow::bail!(
            "cel.iterVar() expects exactly 2 arguments (N, M), got {}",
            call_expr.args.len()
        );
    }
    let n = extract_non_negative_int_arg(call_expr, "cel.iterVar", 0)?;
    let m = extract_non_negative_int_arg(call_expr, "cel.iterVar", 1)?;
    let var_name = format!("@it:{}:{}", n, m);
    compile_ident(&var_name, body, env, ctx, module)
}

/// Compile `cel.accuVar(N, M)` — reference the accumulator variable at
/// comprehension nesting depth N, scope M.
///
/// Rewrites to a lookup of `@ac:N:M`.
pub fn compile_cel_accu_var(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 2 {
        anyhow::bail!(
            "cel.accuVar() expects exactly 2 arguments (N, M), got {}",
            call_expr.args.len()
        );
    }
    let n = extract_non_negative_int_arg(call_expr, "cel.accuVar", 0)?;
    let m = extract_non_negative_int_arg(call_expr, "cel.accuVar", 1)?;
    let var_name = format!("@ac:{}:{}", n, m);
    compile_ident(&var_name, body, env, ctx, module)
}

/// Extract a non-negative integer literal from argument position `pos`.
fn extract_non_negative_int_arg(
    call_expr: &CallExpr,
    func: &str,
    pos: usize,
) -> Result<u64, anyhow::Error> {
    let arg = call_expr.args.get(pos).ok_or_else(|| {
        anyhow::anyhow!(
            "{}() argument {} is missing (got {} args)",
            func,
            pos,
            call_expr.args.len()
        )
    })?;
    match &arg.expr {
        Expr::Literal(lit) => match lit {
            LiteralValue::Int(n) if *n.inner() >= 0 => Ok(*n.inner() as u64),
            LiteralValue::UInt(n) => Ok(*n.inner()),
            _ => anyhow::bail!(
                "{}() argument {} must be a non-negative integer literal",
                func,
                pos
            ),
        },
        _ => anyhow::bail!("{}() argument {} must be an integer literal", func, pos),
    }
}
