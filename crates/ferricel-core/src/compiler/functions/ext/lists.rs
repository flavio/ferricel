//! Compiler for CEL extended list library functions.
//!
//! Handles: `join`, `distinct`, `flatten`, `reverse`, `slice`, `sort`, `sortBy`,
//! `first`, `last`.
//! `lists.range(n)` is dispatched from `mod.rs` with a namespace guard and
//! compiled here via `compile_list_range`.

use cel::common::ast::{CallExpr, Expr};
use ferricel_types::functions::RuntimeFunction;
use walrus::{InstrSeqBuilder, ValType};

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::{compile_call_binary, compile_call_ternary, compile_call_unary},
};

/// Compile an extended list library method call (receiver-style).
pub fn compile_ext_list_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        "join" => compile_join(call_expr, body, env, ctx, module),
        "distinct" => compile_call_unary(
            call_expr,
            "distinct",
            RuntimeFunction::ListDistinct,
            body,
            env,
            ctx,
            module,
        ),
        "flatten" => compile_flatten(call_expr, body, env, ctx, module),
        "reverse" => compile_call_unary(
            call_expr,
            "reverse",
            RuntimeFunction::ListReverse,
            body,
            env,
            ctx,
            module,
        ),
        "slice" => compile_call_ternary(
            call_expr,
            "slice",
            RuntimeFunction::ListSlice,
            body,
            env,
            ctx,
            module,
        ),
        "sort" => compile_call_unary(
            call_expr,
            "sort",
            RuntimeFunction::ListSort,
            body,
            env,
            ctx,
            module,
        ),
        "first" => compile_call_unary(
            call_expr,
            "first",
            RuntimeFunction::ListFirst,
            body,
            env,
            ctx,
            module,
        ),
        "last" => compile_call_unary(
            call_expr,
            "last",
            RuntimeFunction::ListLast,
            body,
            env,
            ctx,
            module,
        ),
        "sortBy" => compile_sort_by(call_expr, body, env, ctx, module),
        _ => anyhow::bail!("Unknown ext list function: {}", func_name),
    }
}

/// Compile `lists.range(n)` — namespace-qualified, no receiver.
///
/// CEL parses `lists.range(n)` as:
///   target = Ident("lists"), func = "range", args = \[n\]
pub fn compile_list_range(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 1 {
        anyhow::bail!("lists.range() expects exactly 1 argument");
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    body.call(env.get(RuntimeFunction::ListRange));
    Ok(())
}

/// Compile `list.join()` or `list.join(sep)`.
fn compile_join(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let arg_count = if call_expr.target.is_some() {
        call_expr.args.len()
    } else {
        call_expr.args.len().saturating_sub(1)
    };

    match arg_count {
        0 => compile_call_unary(
            call_expr,
            "join",
            RuntimeFunction::ListJoin,
            body,
            env,
            ctx,
            module,
        ),
        1 => compile_call_binary(
            call_expr,
            "join",
            RuntimeFunction::ListJoinSep,
            body,
            env,
            ctx,
            module,
        ),
        _ => anyhow::bail!("join() expects 0 or 1 arguments"),
    }
}

/// Compile `list.flatten()` or `list.flatten(depth)`.
fn compile_flatten(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let arg_count = if call_expr.target.is_some() {
        call_expr.args.len()
    } else {
        call_expr.args.len().saturating_sub(1)
    };

    match arg_count {
        0 => compile_call_unary(
            call_expr,
            "flatten",
            RuntimeFunction::ListFlatten,
            body,
            env,
            ctx,
            module,
        ),
        1 => compile_call_binary(
            call_expr,
            "flatten",
            RuntimeFunction::ListFlattenDepth,
            body,
            env,
            ctx,
            module,
        ),
        _ => anyhow::bail!("flatten() expects 0 or 1 arguments"),
    }
}

/// Compile `list.sortBy(var, keyExpr)`.
///
/// CEL form: `list.sortBy(e, keyExpr)`
/// AST: `target=list_expr`, `func="sortBy"`, `args=[Ident("e"), keyExpr]`
///
/// Emits Wasm that:
/// 1. Compiles `list_expr` → `list_local`
/// 2. Creates an empty array → `keys_local`
/// 3. Loops over `list_local`, for each element binds it to `var`, compiles `keyExpr`,
///    and pushes the result into `keys_local`.
/// 4. Calls `cel_list_sort_by_associated_keys(list_local, keys_local)`.
fn compile_sort_by(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let target = call_expr
        .target
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("sortBy() requires a list receiver"))?;

    if call_expr.args.len() != 2 {
        anyhow::bail!("sortBy() expects exactly 2 arguments (var, keyExpr)");
    }

    let var_name = match &call_expr.args[0].expr {
        Expr::Ident(name) => name.clone(),
        _ => anyhow::bail!("sortBy() first argument must be a simple identifier"),
    };
    let key_expr = &call_expr.args[1].expr;

    // 1. Compile the list receiver → store in a local.
    compile_expr(&target.expr, body, env, ctx, module)?;
    let list_local = module.locals.add(ValType::I32);
    body.local_set(list_local);

    // 2. Get the list length.
    body.local_get(list_local);
    body.call(env.get(RuntimeFunction::ArrayLen));
    let len_local = module.locals.add(ValType::I32);
    body.local_set(len_local);

    // 3. Create an empty array for the keys.
    body.call(env.get(RuntimeFunction::CreateArray));
    let keys_local = module.locals.add(ValType::I32);
    body.local_set(keys_local);

    // 4. Loop: for each element compile keyExpr and push into keys array.
    let index_local = module.locals.add(ValType::I32);
    body.i32_const(0);
    body.local_set(index_local);

    let exit_block = body.dangling_instr_seq(None);
    let exit_block_id = exit_block.id();
    let continue_loop = body.dangling_instr_seq(None);
    let continue_loop_id = continue_loop.id();

    body.instr(walrus::ir::Block { seq: exit_block_id });
    body.instr_seq(exit_block_id).instr(walrus::ir::Loop {
        seq: continue_loop_id,
    });

    let mut loop_body = body.instr_seq(continue_loop_id);

    // Exit if index >= len.
    loop_body.local_get(index_local);
    loop_body.local_get(len_local);
    loop_body.binop(walrus::ir::BinaryOp::I32GeU);
    loop_body.instr(walrus::ir::BrIf {
        block: exit_block_id,
    });

    // Get current element.
    loop_body.local_get(list_local);
    loop_body.local_get(index_local);
    loop_body.call(env.get(RuntimeFunction::ArrayGet));
    let elem_local = module.locals.add(ValType::I32);
    loop_body.local_set(elem_local);

    // Compile keyExpr with var bound to the current element.
    let inner_ctx = ctx.with_local(var_name, elem_local);
    compile_expr(key_expr, &mut loop_body, env, &inner_ctx, module)?;

    // Push the key into the keys array. cel_array_push(array_ptr, element_ptr) → void.
    let key_result_local = module.locals.add(ValType::I32);
    loop_body.local_set(key_result_local);
    loop_body.local_get(keys_local);
    loop_body.local_get(key_result_local);
    loop_body.call(env.get(RuntimeFunction::ArrayPush));
    // ArrayPush is void — keys_local is mutated in place, no return value to store.

    // Increment index and loop.
    loop_body.local_get(index_local);
    loop_body.i32_const(1);
    loop_body.binop(walrus::ir::BinaryOp::I32Add);
    loop_body.local_set(index_local);
    loop_body.instr(walrus::ir::Br {
        block: continue_loop_id,
    });

    // 5. Call sortByAssociatedKeys(list, keys).
    body.local_get(list_local);
    body.local_get(keys_local);
    body.call(env.get(RuntimeFunction::ListSortByAssociatedKeys));

    Ok(())
}
