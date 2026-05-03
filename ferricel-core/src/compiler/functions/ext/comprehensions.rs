//! Compiler support for two-variable comprehension macros.
//!
//! These macros are NOT handled by the `cel` parser (v0.13.0), which only supports
//! single-variable comprehensions. The parser emits them as regular `Call` nodes, so
//! we intercept them here and compile them by emitting Wasm loops directly.
//!
//! # Supported macros
//!
//! | Macro | Form |
//! |-------|------|
//! | `list.exists(i, v, pred)` | Short-circuit OR over list elements |
//! | `map.exists(k, v, pred)` | Short-circuit OR over map entries |
//! | `list.all(i, v, pred)` | Short-circuit AND over list elements |
//! | `map.all(k, v, pred)` | Short-circuit AND over map entries |
//! | `list.existsOne(i, v, pred)` | Exactly one match |
//! | `map.existsOne(k, v, pred)` | Exactly one match |
//! | `list.transformList(i, v, expr)` | Map list → list |
//! | `list.transformList(i, v, filter, expr)` | Filtered map list → list |
//! | `map.transformList(k, v, expr)` | Map map-entries → list |
//! | `map.transformList(k, v, filter, expr)` | Filtered map map-entries → list |
//! | `list.transformMap(i, v, expr)` | Map list → map (key = index) |
//! | `list.transformMap(i, v, filter, expr)` | Filtered map list → map |
//! | `map.transformMap(k, v, expr)` | Map map-entries → map (same keys, new values) |
//! | `map.transformMap(k, v, filter, expr)` | Filtered map map-entries → map |
//!
//! # Loop structure
//!
//! All macros share the same two-variable loop skeleton:
//! ```text
//! range    = compile(target)
//! prepared = cel_iter_prepare(range)      // list→self, map→keys array
//! len      = cel_array_len(prepared)
//! accu     = <initial accumulator>
//! index    = 0
//! block $exit {
//!   loop $continue {
//!     if index >= len: br $exit
//!     var1 = cel_iter_var1(range, prepared, index)  // list→Int(idx), map→key
//!     var2 = cel_iter_var2(range, prepared, index)  // list→element, map→value
//!     <body using var1, var2, accu>
//!     index += 1
//!     br $continue
//!   }
//! }
//! <result using accu>
//! ```

use cel::common::ast::{CallExpr, Expr};
use ferricel_types::functions::RuntimeFunction;
use walrus::ir::InstrSeqId;
use walrus::{InstrSeqBuilder, ValType};

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
};

/// Compile a two-variable comprehension call.
///
/// Dispatches to the appropriate helper based on `func_name` and argument count.
pub fn compile_two_var_comprehension(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        "exists" => compile_exists(call_expr, body, env, ctx, module),
        "all" => compile_all(call_expr, body, env, ctx, module),
        "existsOne" | "exists_one" => compile_exists_one(call_expr, body, env, ctx, module),
        "transformList" => compile_transform_list(call_expr, body, env, ctx, module),
        "transformMap" => compile_transform_map(call_expr, body, env, ctx, module),
        "transformMapEntry" => compile_transform_map_entry(call_expr, body, env, ctx, module),
        _ => anyhow::bail!("Unknown two-var comprehension macro: {func_name}"),
    }
}

/// Extract and validate the two variable names from args[0] and args[1].
fn extract_var_names(
    call_expr: &CallExpr,
    macro_name: &str,
) -> Result<(String, String), anyhow::Error> {
    let var1 = match &call_expr.args[0].expr {
        Expr::Ident(name) => name.clone(),
        _ => anyhow::bail!("{macro_name}: first argument must be a simple identifier"),
    };
    let var2 = match &call_expr.args[1].expr {
        Expr::Ident(name) => name.clone(),
        _ => anyhow::bail!("{macro_name}: second argument must be a simple identifier"),
    };
    Ok((var1, var2))
}

/// Shared loop setup: compiles the range, calls IterPrepare, gets length.
///
/// After this function returns:
/// - `range_local`: holds the original range pointer (list or map)
/// - `prepared_local`: holds the result of `cel_iter_prepare(range)` (always an array for `cel_array_len`)
/// - `length_local`: holds the i32 iteration count
/// - `index_local`: initialized to 0
fn emit_loop_setup(
    target: &cel::common::ast::Expr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<
    (
        walrus::LocalId,
        walrus::LocalId,
        walrus::LocalId,
        walrus::LocalId,
    ),
    anyhow::Error,
> {
    // 1. Compile the receiver (range)
    compile_expr(target, body, env, ctx, module)?;
    let range_local = module.locals.add(ValType::I32);
    body.local_set(range_local);

    // 2. IterPrepare → prepared (array of keys for maps, self for lists)
    body.local_get(range_local);
    body.call(env.get(RuntimeFunction::IterPrepare));
    let prepared_local = module.locals.add(ValType::I32);
    body.local_set(prepared_local);

    // 3. Get iteration length
    body.local_get(prepared_local);
    body.call(env.get(RuntimeFunction::ArrayLen));
    let length_local = module.locals.add(ValType::I32);
    body.local_set(length_local);

    // 4. Index starts at 0
    let index_local = module.locals.add(ValType::I32);
    body.i32_const(0);
    body.local_set(index_local);

    Ok((range_local, prepared_local, length_local, index_local))
}

/// Bind var1 and var2 in the loop body.
///
/// Returns `(var1_local, var2_local, inner_ctx)`.
#[allow(clippy::too_many_arguments)]
fn emit_bind_iter_vars(
    range_local: walrus::LocalId,
    prepared_local: walrus::LocalId,
    index_local: walrus::LocalId,
    var1_name: &str,
    var2_name: &str,
    loop_body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> CompilerContext {
    // var1 = cel_iter_var1(range, prepared, index)
    loop_body.local_get(range_local);
    loop_body.local_get(prepared_local);
    loop_body.local_get(index_local);
    loop_body.call(env.get(RuntimeFunction::IterVar1));
    let var1_local = module.locals.add(ValType::I32);
    loop_body.local_set(var1_local);

    // var2 = cel_iter_var2(range, prepared, index)
    loop_body.local_get(range_local);
    loop_body.local_get(prepared_local);
    loop_body.local_get(index_local);
    loop_body.call(env.get(RuntimeFunction::IterVar2));
    let var2_local = module.locals.add(ValType::I32);
    loop_body.local_set(var2_local);

    ctx.with_local(var1_name.to_string(), var1_local)
        .with_local(var2_name.to_string(), var2_local)
}

/// Emit the standard exit-check + index increment + br-to-loop instructions.
fn emit_loop_exit_check(
    index_local: walrus::LocalId,
    length_local: walrus::LocalId,
    exit_block_id: InstrSeqId,
    loop_body: &mut InstrSeqBuilder,
) {
    loop_body.local_get(index_local);
    loop_body.local_get(length_local);
    loop_body.binop(walrus::ir::BinaryOp::I32GeU);
    loop_body.instr(walrus::ir::BrIf {
        block: exit_block_id,
    });
}

fn emit_index_increment_and_loop(
    index_local: walrus::LocalId,
    continue_loop_id: InstrSeqId,
    loop_body: &mut InstrSeqBuilder,
) {
    loop_body.local_get(index_local);
    loop_body.i32_const(1);
    loop_body.binop(walrus::ir::BinaryOp::I32Add);
    loop_body.local_set(index_local);
    loop_body.instr(walrus::ir::Br {
        block: continue_loop_id,
    });
}

// ─── exists(i, v, pred) ────────────────────────────────────────────────────

/// Compile `range.exists(var1, var2, predicate)`.
///
/// Evaluates to `true` if `predicate` is true for any element/entry.
/// Short-circuits (exits early) as soon as the accumulator becomes strictly true.
/// Error semantics: `error || false = error`, `error || true = true`.
fn compile_exists(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 3 {
        anyhow::bail!(
            "exists() two-var form expects 3 arguments (var1, var2, pred), got {}",
            call_expr.args.len()
        );
    }
    let target = call_expr
        .target
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("exists() requires a receiver"))?;
    let (var1, var2) = extract_var_names(call_expr, "exists")?;
    let pred_expr = &call_expr.args[2].expr;

    let (range_local, prepared_local, length_local, index_local) =
        emit_loop_setup(&target.expr, body, env, ctx, module)?;

    // accu = Bool(false)
    body.i64_const(0);
    body.call(env.get(RuntimeFunction::CreateBool));
    let accu_local = module.locals.add(ValType::I32);
    body.local_set(accu_local);

    let exit_block = body.dangling_instr_seq(None);
    let exit_block_id = exit_block.id();
    let continue_loop = body.dangling_instr_seq(None);
    let continue_loop_id = continue_loop.id();

    body.instr(walrus::ir::Block { seq: exit_block_id });
    body.instr_seq(exit_block_id).instr(walrus::ir::Loop {
        seq: continue_loop_id,
    });

    let mut lb = body.instr_seq(continue_loop_id);

    emit_loop_exit_check(index_local, length_local, exit_block_id, &mut lb);

    let inner_ctx = emit_bind_iter_vars(
        range_local,
        prepared_local,
        index_local,
        &var1,
        &var2,
        &mut lb,
        env,
        ctx,
        module,
    );

    // accu = cel_bool_or(accu, pred)
    lb.local_get(accu_local);
    compile_expr(pred_expr, &mut lb, env, &inner_ctx, module)?;
    lb.call(env.get(RuntimeFunction::BoolOr));
    lb.local_set(accu_local);

    // Short-circuit: if accu is strictly true, exit
    lb.local_get(accu_local);
    lb.call(env.get(RuntimeFunction::IsStrictlyTrue));
    lb.instr(walrus::ir::BrIf {
        block: exit_block_id,
    });

    emit_index_increment_and_loop(index_local, continue_loop_id, &mut lb);

    // Result: accu
    body.local_get(accu_local);
    Ok(())
}

// ─── all(i, v, pred) ───────────────────────────────────────────────────────

/// Compile `range.all(var1, var2, predicate)`.
///
/// Evaluates to `true` if `predicate` is true for every element/entry.
/// Short-circuits as soon as the accumulator becomes strictly false.
fn compile_all(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 3 {
        anyhow::bail!(
            "all() two-var form expects 3 arguments (var1, var2, pred), got {}",
            call_expr.args.len()
        );
    }
    let target = call_expr
        .target
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("all() requires a receiver"))?;
    let (var1, var2) = extract_var_names(call_expr, "all")?;
    let pred_expr = &call_expr.args[2].expr;

    let (range_local, prepared_local, length_local, index_local) =
        emit_loop_setup(&target.expr, body, env, ctx, module)?;

    // accu = Bool(true)
    body.i64_const(1);
    body.call(env.get(RuntimeFunction::CreateBool));
    let accu_local = module.locals.add(ValType::I32);
    body.local_set(accu_local);

    let exit_block = body.dangling_instr_seq(None);
    let exit_block_id = exit_block.id();
    let continue_loop = body.dangling_instr_seq(None);
    let continue_loop_id = continue_loop.id();

    body.instr(walrus::ir::Block { seq: exit_block_id });
    body.instr_seq(exit_block_id).instr(walrus::ir::Loop {
        seq: continue_loop_id,
    });

    let mut lb = body.instr_seq(continue_loop_id);

    emit_loop_exit_check(index_local, length_local, exit_block_id, &mut lb);

    let inner_ctx = emit_bind_iter_vars(
        range_local,
        prepared_local,
        index_local,
        &var1,
        &var2,
        &mut lb,
        env,
        ctx,
        module,
    );

    // accu = cel_bool_and(accu, pred)
    lb.local_get(accu_local);
    compile_expr(pred_expr, &mut lb, env, &inner_ctx, module)?;
    lb.call(env.get(RuntimeFunction::BoolAnd));
    lb.local_set(accu_local);

    // Short-circuit: if accu is strictly false, exit
    lb.local_get(accu_local);
    lb.call(env.get(RuntimeFunction::IsStrictlyFalse));
    lb.instr(walrus::ir::BrIf {
        block: exit_block_id,
    });

    emit_index_increment_and_loop(index_local, continue_loop_id, &mut lb);

    // Result: accu
    body.local_get(accu_local);
    Ok(())
}

// ─── existsOne(i, v, pred) ─────────────────────────────────────────────────

/// Compile `range.existsOne(var1, var2, predicate)`.
///
/// Evaluates to `true` if exactly one element/entry satisfies `predicate`.
/// No short-circuiting — all elements are always evaluated.
fn compile_exists_one(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 3 {
        anyhow::bail!(
            "existsOne() two-var form expects 3 arguments (var1, var2, pred), got {}",
            call_expr.args.len()
        );
    }
    let target = call_expr
        .target
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("existsOne() requires a receiver"))?;
    let (var1, var2) = extract_var_names(call_expr, "existsOne")?;
    let pred_expr = &call_expr.args[2].expr;

    let (range_local, prepared_local, length_local, index_local) =
        emit_loop_setup(&target.expr, body, env, ctx, module)?;

    // accu = Int(0)
    body.i64_const(0);
    body.call(env.get(RuntimeFunction::CreateInt));
    let accu_local = module.locals.add(ValType::I32);
    body.local_set(accu_local);

    let exit_block = body.dangling_instr_seq(None);
    let exit_block_id = exit_block.id();
    let continue_loop = body.dangling_instr_seq(None);
    let continue_loop_id = continue_loop.id();

    body.instr(walrus::ir::Block { seq: exit_block_id });
    body.instr_seq(exit_block_id).instr(walrus::ir::Loop {
        seq: continue_loop_id,
    });

    let mut lb = body.instr_seq(continue_loop_id);

    emit_loop_exit_check(index_local, length_local, exit_block_id, &mut lb);

    let inner_ctx = emit_bind_iter_vars(
        range_local,
        prepared_local,
        index_local,
        &var1,
        &var2,
        &mut lb,
        env,
        ctx,
        module,
    );

    // accu = cel_cond_inc(accu, pred)
    lb.local_get(accu_local);
    compile_expr(pred_expr, &mut lb, env, &inner_ctx, module)?;
    lb.call(env.get(RuntimeFunction::CondInc));
    lb.local_set(accu_local);

    // No short-circuit for existsOne

    emit_index_increment_and_loop(index_local, continue_loop_id, &mut lb);

    // Result: accu == Int(1)
    // We need to compare accu (which is *CelValue::Int) to Int(1).
    // Emit: cel_value_eq(accu, CreateInt(1))
    body.local_get(accu_local);
    body.i64_const(1);
    body.call(env.get(RuntimeFunction::CreateInt));
    body.call(env.get(RuntimeFunction::ValueEq));
    Ok(())
}

// ─── transformList(i, v, expr) and transformList(i, v, filter, expr) ───────

/// Compile `range.transformList(var1, var2, transform_expr)` (3-arg form)
/// or      `range.transformList(var1, var2, filter_expr, transform_expr)` (4-arg form).
///
/// Returns a new list containing the results of `transform_expr` for each element,
/// optionally filtered by `filter_expr`.
fn compile_transform_list(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let nargs = call_expr.args.len();
    if nargs != 3 && nargs != 4 {
        anyhow::bail!("transformList() expects 3 or 4 arguments, got {nargs}");
    }
    let target = call_expr
        .target
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("transformList() requires a receiver"))?;
    let (var1, var2) = extract_var_names(call_expr, "transformList")?;
    let (filter_expr, transform_expr) = if nargs == 4 {
        (Some(&call_expr.args[2].expr), &call_expr.args[3].expr)
    } else {
        (None, &call_expr.args[2].expr)
    };

    let (range_local, prepared_local, length_local, index_local) =
        emit_loop_setup(&target.expr, body, env, ctx, module)?;

    // accu = [] (empty array)
    body.call(env.get(RuntimeFunction::CreateArray));
    let accu_local = module.locals.add(ValType::I32);
    body.local_set(accu_local);

    let exit_block = body.dangling_instr_seq(None);
    let exit_block_id = exit_block.id();
    let continue_loop = body.dangling_instr_seq(None);
    let continue_loop_id = continue_loop.id();

    body.instr(walrus::ir::Block { seq: exit_block_id });
    body.instr_seq(exit_block_id).instr(walrus::ir::Loop {
        seq: continue_loop_id,
    });

    let mut lb = body.instr_seq(continue_loop_id);

    emit_loop_exit_check(index_local, length_local, exit_block_id, &mut lb);

    let inner_ctx = emit_bind_iter_vars(
        range_local,
        prepared_local,
        index_local,
        &var1,
        &var2,
        &mut lb,
        env,
        ctx,
        module,
    );
    let inner_ctx = inner_ctx.with_local("@accu".to_string(), accu_local);

    // Shared temp local for holding intermediate results before error-checking.
    let result_local = module.locals.add(ValType::I32);

    if let Some(filt) = filter_expr {
        // Evaluate filter; propagate error immediately.
        compile_expr(filt, &mut lb, env, &inner_ctx, module)?;
        lb.local_set(result_local);
        lb.local_get(result_local);
        lb.call(env.get(RuntimeFunction::IsError));
        let ferr_then = lb.dangling_instr_seq(None);
        let ferr_then_id = ferr_then.id();
        let ferr_else = lb.dangling_instr_seq(None);
        let ferr_else_id = ferr_else.id();
        {
            let mut t = lb.instr_seq(ferr_then_id);
            t.local_get(result_local);
            t.local_set(accu_local);
            t.instr(walrus::ir::Br {
                block: exit_block_id,
            });
        }
        {
            // else: check if strictly true, then conditionally push transform
            let mut e = lb.instr_seq(ferr_else_id);
            e.local_get(result_local);
            e.call(env.get(RuntimeFunction::IsStrictlyTrue));

            let then_seq = e.dangling_instr_seq(None);
            let then_id = then_seq.id();
            let else_seq = e.dangling_instr_seq(None);
            let else_id = else_seq.id();

            {
                // then: evaluate transform, check for error, then push
                let mut then_body = e.instr_seq(then_id);
                compile_expr(transform_expr, &mut then_body, env, &inner_ctx, module)?;
                then_body.local_set(result_local);
                then_body.local_get(result_local);
                then_body.call(env.get(RuntimeFunction::IsError));
                let terr_then = then_body.dangling_instr_seq(None);
                let terr_then_id = terr_then.id();
                let terr_else = then_body.dangling_instr_seq(None);
                let terr_else_id = terr_else.id();
                {
                    let mut tt = then_body.instr_seq(terr_then_id);
                    tt.local_get(result_local);
                    tt.local_set(accu_local);
                    tt.instr(walrus::ir::Br {
                        block: exit_block_id,
                    });
                }
                {
                    let mut te = then_body.instr_seq(terr_else_id);
                    te.local_get(accu_local);
                    te.local_get(result_local);
                    te.call(env.get(RuntimeFunction::ArrayPush));
                }
                then_body.instr(walrus::ir::IfElse {
                    consequent: terr_then_id,
                    alternative: terr_else_id,
                });
            }
            {
                let _ = e.instr_seq(else_id); // no-op
            }
            e.instr(walrus::ir::IfElse {
                consequent: then_id,
                alternative: else_id,
            });
        }
        lb.instr(walrus::ir::IfElse {
            consequent: ferr_then_id,
            alternative: ferr_else_id,
        });
    } else {
        // Unconditional push — evaluate transform, check for error first.
        compile_expr(transform_expr, &mut lb, env, &inner_ctx, module)?;
        lb.local_set(result_local);
        lb.local_get(result_local);
        lb.call(env.get(RuntimeFunction::IsError));
        let err_then = lb.dangling_instr_seq(None);
        let err_then_id = err_then.id();
        let err_else = lb.dangling_instr_seq(None);
        let err_else_id = err_else.id();
        {
            let mut t = lb.instr_seq(err_then_id);
            t.local_get(result_local);
            t.local_set(accu_local);
            t.instr(walrus::ir::Br {
                block: exit_block_id,
            });
        }
        {
            let mut e = lb.instr_seq(err_else_id);
            e.local_get(accu_local);
            e.local_get(result_local);
            e.call(env.get(RuntimeFunction::ArrayPush));
        }
        lb.instr(walrus::ir::IfElse {
            consequent: err_then_id,
            alternative: err_else_id,
        });
    }

    emit_index_increment_and_loop(index_local, continue_loop_id, &mut lb);

    // Result: accu (the accumulated map)
    body.local_get(accu_local);
    Ok(())
}

// ─── transformMap(k, v, expr) and transformMap(k, v, filter, expr) ─────────

/// Compile `range.transformMap(var1, var2, transform_expr)` (3-arg form)
/// or      `range.transformMap(var1, var2, filter_expr, transform_expr)` (4-arg form).
///
/// For **list** receivers: returns a map where key = index (Int), value = transform(index, element).
/// For **map** receivers: returns a new map with the same keys but values replaced by transform(key, value).
fn compile_transform_map(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let nargs = call_expr.args.len();
    if nargs != 3 && nargs != 4 {
        anyhow::bail!("transformMap() expects 3 or 4 arguments, got {nargs}");
    }
    let target = call_expr
        .target
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("transformMap() requires a receiver"))?;
    let (var1, var2) = extract_var_names(call_expr, "transformMap")?;
    let (filter_expr, transform_expr) = if nargs == 4 {
        (Some(&call_expr.args[2].expr), &call_expr.args[3].expr)
    } else {
        (None, &call_expr.args[2].expr)
    };

    let (range_local, prepared_local, length_local, index_local) =
        emit_loop_setup(&target.expr, body, env, ctx, module)?;

    // accu = {} (empty map)
    body.call(env.get(RuntimeFunction::CreateMap));
    let accu_local = module.locals.add(ValType::I32);
    body.local_set(accu_local);

    let exit_block = body.dangling_instr_seq(None);
    let exit_block_id = exit_block.id();
    let continue_loop = body.dangling_instr_seq(None);
    let continue_loop_id = continue_loop.id();

    body.instr(walrus::ir::Block { seq: exit_block_id });
    body.instr_seq(exit_block_id).instr(walrus::ir::Loop {
        seq: continue_loop_id,
    });

    let mut lb = body.instr_seq(continue_loop_id);

    emit_loop_exit_check(index_local, length_local, exit_block_id, &mut lb);

    let inner_ctx = emit_bind_iter_vars(
        range_local,
        prepared_local,
        index_local,
        &var1,
        &var2,
        &mut lb,
        env,
        ctx,
        module,
    );
    let inner_ctx = inner_ctx.with_local("@accu".to_string(), accu_local);

    // Shared temp local for error-checking intermediate results.
    let result_local = module.locals.add(ValType::I32);

    if let Some(filt) = filter_expr {
        // Evaluate filter; propagate error immediately.
        compile_expr(filt, &mut lb, env, &inner_ctx, module)?;
        lb.local_set(result_local);
        lb.local_get(result_local);
        lb.call(env.get(RuntimeFunction::IsError));
        let ferr_then = lb.dangling_instr_seq(None);
        let ferr_then_id = ferr_then.id();
        let ferr_else = lb.dangling_instr_seq(None);
        let ferr_else_id = ferr_else.id();
        {
            let mut t = lb.instr_seq(ferr_then_id);
            t.local_get(result_local);
            t.local_set(accu_local);
            t.instr(walrus::ir::Br {
                block: exit_block_id,
            });
        }
        {
            let mut e = lb.instr_seq(ferr_else_id);
            e.local_get(result_local);
            e.call(env.get(RuntimeFunction::IsStrictlyTrue));

            let then_seq = e.dangling_instr_seq(None);
            let then_id = then_seq.id();
            let else_seq = e.dangling_instr_seq(None);
            let else_id = else_seq.id();

            {
                // then: evaluate transform, check error, then insert
                let mut then_body = e.instr_seq(then_id);
                compile_expr(transform_expr, &mut then_body, env, &inner_ctx, module)?;
                then_body.local_set(result_local);
                then_body.local_get(result_local);
                then_body.call(env.get(RuntimeFunction::IsError));
                let terr_then = then_body.dangling_instr_seq(None);
                let terr_then_id = terr_then.id();
                let terr_else = then_body.dangling_instr_seq(None);
                let terr_else_id = terr_else.id();
                {
                    let mut tt = then_body.instr_seq(terr_then_id);
                    tt.local_get(result_local);
                    tt.local_set(accu_local);
                    tt.instr(walrus::ir::Br {
                        block: exit_block_id,
                    });
                }
                {
                    let mut te = then_body.instr_seq(terr_else_id);
                    te.local_get(accu_local);
                    compile_expr(&Expr::Ident(var1.clone()), &mut te, env, &inner_ctx, module)?;
                    te.local_get(result_local);
                    te.call(env.get(RuntimeFunction::MapInsert));
                }
                then_body.instr(walrus::ir::IfElse {
                    consequent: terr_then_id,
                    alternative: terr_else_id,
                });
            }
            {
                let _ = e.instr_seq(else_id);
            }
            e.instr(walrus::ir::IfElse {
                consequent: then_id,
                alternative: else_id,
            });
        }
        lb.instr(walrus::ir::IfElse {
            consequent: ferr_then_id,
            alternative: ferr_else_id,
        });
    } else {
        // Unconditional insert — evaluate transform, check for error first.
        compile_expr(transform_expr, &mut lb, env, &inner_ctx, module)?;
        lb.local_set(result_local);
        lb.local_get(result_local);
        lb.call(env.get(RuntimeFunction::IsError));
        let err_then = lb.dangling_instr_seq(None);
        let err_then_id = err_then.id();
        let err_else = lb.dangling_instr_seq(None);
        let err_else_id = err_else.id();
        {
            let mut t = lb.instr_seq(err_then_id);
            t.local_get(result_local);
            t.local_set(accu_local);
            t.instr(walrus::ir::Br {
                block: exit_block_id,
            });
        }
        {
            let mut e = lb.instr_seq(err_else_id);
            e.local_get(accu_local);
            compile_expr(&Expr::Ident(var1.clone()), &mut e, env, &inner_ctx, module)?;
            e.local_get(result_local);
            e.call(env.get(RuntimeFunction::MapInsert));
        }
        lb.instr(walrus::ir::IfElse {
            consequent: err_then_id,
            alternative: err_else_id,
        });
    }

    emit_index_increment_and_loop(index_local, continue_loop_id, &mut lb);

    // Result: accu (the accumulated map)
    body.local_get(accu_local);
    Ok(())
}

// ─── transformMapEntry(k, v, entry_expr) and transformMapEntry(k, v, filter, entry_expr) ─

/// Compile `range.transformMapEntry(var1, var2, entry_expr)` (3-arg form)
/// or      `range.transformMapEntry(var1, var2, filter_expr, entry_expr)` (4-arg form).
///
/// The `entry_expr` must evaluate to a map literal. All entries from that map are merged
/// into the output map. Duplicate keys across iterations produce a runtime error.
fn compile_transform_map_entry(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let nargs = call_expr.args.len();
    if nargs != 3 && nargs != 4 {
        anyhow::bail!("transformMapEntry() expects 3 or 4 arguments, got {nargs}");
    }
    let target = call_expr
        .target
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("transformMapEntry() requires a receiver"))?;
    let (var1, var2) = extract_var_names(call_expr, "transformMapEntry")?;
    let (filter_expr, entry_expr) = if nargs == 4 {
        (Some(&call_expr.args[2].expr), &call_expr.args[3].expr)
    } else {
        (None, &call_expr.args[2].expr)
    };

    let (range_local, prepared_local, length_local, index_local) =
        emit_loop_setup(&target.expr, body, env, ctx, module)?;

    // accu = {} (empty map)
    body.call(env.get(RuntimeFunction::CreateMap));
    let accu_local = module.locals.add(walrus::ValType::I32);
    body.local_set(accu_local);

    let exit_block = body.dangling_instr_seq(None);
    let exit_block_id = exit_block.id();
    let continue_loop = body.dangling_instr_seq(None);
    let continue_loop_id = continue_loop.id();

    body.instr(walrus::ir::Block { seq: exit_block_id });
    body.instr_seq(exit_block_id).instr(walrus::ir::Loop {
        seq: continue_loop_id,
    });

    let mut lb = body.instr_seq(continue_loop_id);

    emit_loop_exit_check(index_local, length_local, exit_block_id, &mut lb);

    let inner_ctx = emit_bind_iter_vars(
        range_local,
        prepared_local,
        index_local,
        &var1,
        &var2,
        &mut lb,
        env,
        ctx,
        module,
    );
    let inner_ctx = inner_ctx.with_local("@accu".to_string(), accu_local);

    // Shared temp local for error-checking intermediate results.
    let result_local = module.locals.add(walrus::ValType::I32);

    // Emit: evaluate entry_expr, check for error, then call MapInsertEntry(accu, entry).
    // MapInsertEntry returns accu (success) or error (duplicate key).
    // Store result back into accu_local; if error, br $exit.
    macro_rules! emit_insert_entry {
        ($seq:expr) => {{
            let seq: &mut InstrSeqBuilder = $seq;
            compile_expr(entry_expr, seq, env, &inner_ctx, module)?;
            seq.local_set(result_local);
            // Check if entry expression itself errored
            seq.local_get(result_local);
            seq.call(env.get(RuntimeFunction::IsError));
            let eerr_then = seq.dangling_instr_seq(None);
            let eerr_then_id = eerr_then.id();
            let eerr_else = seq.dangling_instr_seq(None);
            let eerr_else_id = eerr_else.id();
            {
                let mut t = seq.instr_seq(eerr_then_id);
                t.local_get(result_local);
                t.local_set(accu_local);
                t.instr(walrus::ir::Br {
                    block: exit_block_id,
                });
            }
            {
                // else: call MapInsertEntry(accu, entry) → new accu (or error)
                let mut e = seq.instr_seq(eerr_else_id);
                e.local_get(accu_local);
                e.local_get(result_local);
                e.call(env.get(RuntimeFunction::MapInsertEntry));
                e.local_set(accu_local);
                // Check if MapInsertEntry returned an error (duplicate key)
                e.local_get(accu_local);
                e.call(env.get(RuntimeFunction::IsError));
                e.instr(walrus::ir::BrIf {
                    block: exit_block_id,
                });
            }
            seq.instr(walrus::ir::IfElse {
                consequent: eerr_then_id,
                alternative: eerr_else_id,
            });
        }};
    }

    if let Some(filt) = filter_expr {
        // Evaluate filter; propagate error immediately.
        compile_expr(filt, &mut lb, env, &inner_ctx, module)?;
        lb.local_set(result_local);
        lb.local_get(result_local);
        lb.call(env.get(RuntimeFunction::IsError));
        let ferr_then = lb.dangling_instr_seq(None);
        let ferr_then_id = ferr_then.id();
        let ferr_else = lb.dangling_instr_seq(None);
        let ferr_else_id = ferr_else.id();
        {
            let mut t = lb.instr_seq(ferr_then_id);
            t.local_get(result_local);
            t.local_set(accu_local);
            t.instr(walrus::ir::Br {
                block: exit_block_id,
            });
        }
        {
            let mut e = lb.instr_seq(ferr_else_id);
            e.local_get(result_local);
            e.call(env.get(RuntimeFunction::IsStrictlyTrue));

            let then_seq = e.dangling_instr_seq(None);
            let then_id = then_seq.id();
            let else_seq = e.dangling_instr_seq(None);
            let else_id = else_seq.id();

            {
                let mut then_body = e.instr_seq(then_id);
                emit_insert_entry!(&mut then_body);
            }
            {
                let _ = e.instr_seq(else_id);
            }
            e.instr(walrus::ir::IfElse {
                consequent: then_id,
                alternative: else_id,
            });
        }
        lb.instr(walrus::ir::IfElse {
            consequent: ferr_then_id,
            alternative: ferr_else_id,
        });
    } else {
        emit_insert_entry!(&mut lb);
    }

    emit_index_increment_and_loop(index_local, continue_loop_id, &mut lb);

    // Result: accu (the accumulated map)
    body.local_get(accu_local);
    Ok(())
}
