use cel::common::ast::{CallExpr, operators};
use ferricel_types::functions::RuntimeFunction;
use walrus::{InstrSeqBuilder, ValType};

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers,
};

/// Compile a binary operator: validate 2 args, compile both, call a runtime function.
pub(crate) fn compile_binary_op(
    call_expr: &CallExpr,
    op_name: &str,
    runtime_fn: RuntimeFunction,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 2 {
        anyhow::bail!("{} operator expects 2 arguments", op_name);
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
    body.call(env.get(runtime_fn));
    Ok(())
}

/// Compile a unary operator: validate 1 arg, compile it, call a runtime function.
pub(crate) fn compile_unary_op(
    call_expr: &CallExpr,
    op_name: &str,
    runtime_fn: RuntimeFunction,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 1 {
        anyhow::bail!("{} operator expects 1 argument", op_name);
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    body.call(env.get(runtime_fn));
    Ok(())
}

/// Compile logical AND with short-circuit evaluation.
///
/// CEL AND semantics: false && <anything> => false (errors absorbed)
///                    true && <error> => <error> (error propagates)
fn compile_logical_and(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 2 {
        anyhow::bail!("Logical AND operator expects 2 arguments");
    }

    // Compile left operand
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;

    // Create a local variable to store the left result
    let left_local = module.locals.add(ValType::I32); // *mut CelValue is i32
    body.local_tee(left_local); // Duplicate and store left value

    // Check if left is strictly false
    body.call(env.get(RuntimeFunction::IsStrictlyFalse)); // Returns i32: 1 if false, 0 otherwise

    // Create dangling instruction sequences for then and else branches
    // Both branches must produce *mut CelValue (i32) on the stack
    let then_seq = body.dangling_instr_seq(Some(ValType::I32));
    let then_id = then_seq.id();
    let else_seq = body.dangling_instr_seq(Some(ValType::I32));
    let else_id = else_seq.id();

    // Generate the if/else instruction
    body.instr(walrus::ir::IfElse {
        consequent: then_id,
        alternative: else_id,
    });

    // Then branch: return left (which is false)
    body.instr_seq(then_id).local_get(left_local);

    // Else branch: evaluate right and call cel_bool_and
    let mut else_body = body.instr_seq(else_id);
    compile_expr(&call_expr.args[1].expr, &mut else_body, env, ctx, module)?;
    let right_local = module.locals.add(ValType::I32);
    else_body.local_set(right_local); // Store right, consumes stack
    else_body.local_get(left_local); // Push left
    else_body.local_get(right_local); // Push right
    else_body.call(env.get(RuntimeFunction::BoolAnd)); // Call and(left, right)

    Ok(())
}

/// Compile logical OR with short-circuit evaluation.
///
/// CEL OR semantics: true || <anything> => true (errors absorbed)
///                   false || <error> => <error> (error propagates)
fn compile_logical_or(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 2 {
        anyhow::bail!("Logical OR operator expects 2 arguments");
    }

    // Compile left operand
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;

    // Create a local variable to store the left result
    let left_local = module.locals.add(ValType::I32); // *mut CelValue is i32
    body.local_tee(left_local); // Duplicate and store left value

    // Check if left is strictly true
    body.call(env.get(RuntimeFunction::IsStrictlyTrue)); // Returns i32: 1 if true, 0 otherwise

    // Create dangling instruction sequences for then and else branches
    // Both branches must produce *mut CelValue (i32) on the stack
    let then_seq = body.dangling_instr_seq(Some(ValType::I32));
    let then_id = then_seq.id();
    let else_seq = body.dangling_instr_seq(Some(ValType::I32));
    let else_id = else_seq.id();

    // Generate the if/else instruction
    body.instr(walrus::ir::IfElse {
        consequent: then_id,
        alternative: else_id,
    });

    // Then branch: return left (which is true)
    body.instr_seq(then_id).local_get(left_local);

    // Else branch: evaluate right and call cel_bool_or
    let mut else_body = body.instr_seq(else_id);
    compile_expr(&call_expr.args[1].expr, &mut else_body, env, ctx, module)?;
    let right_local = module.locals.add(ValType::I32);
    else_body.local_set(right_local); // Store right, consumes stack
    else_body.local_get(left_local); // Push left
    else_body.local_get(right_local); // Push right
    else_body.call(env.get(RuntimeFunction::BoolOr)); // Call or(left, right)

    Ok(())
}

/// Compile the ternary conditional operator: condition ? true_value : false_value
///
/// CEL semantics: if condition is error, return error.
/// Otherwise evaluate only the appropriate branch.
fn compile_conditional(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 3 {
        anyhow::bail!("Conditional operator expects 3 arguments");
    }

    // Compile condition
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;

    // Store condition in local
    let cond_local = module.locals.add(ValType::I32);
    body.local_tee(cond_local); // Duplicate and store

    // Check if condition is error
    body.call(env.get(RuntimeFunction::IsError)); // Returns i32: 1 if error, 0 otherwise

    // Create sequences for error check
    let is_error_seq = body.dangling_instr_seq(Some(ValType::I32));
    let is_error_id = is_error_seq.id();
    let not_error_seq = body.dangling_instr_seq(Some(ValType::I32));
    let not_error_id = not_error_seq.id();

    body.instr(walrus::ir::IfElse {
        consequent: is_error_id,
        alternative: not_error_id,
    });

    // If condition is error, return it
    body.instr_seq(is_error_id).local_get(cond_local);

    // If condition is not error, convert to bool and branch
    let mut not_error_body = body.instr_seq(not_error_id);
    not_error_body.local_get(cond_local);
    not_error_body.call(env.get(RuntimeFunction::ValueToBool)); // Returns i64: 1 for true, 0 for false
    not_error_body.unop(walrus::ir::UnaryOp::I32WrapI64);

    // Create sequences for true/false branches
    let true_branch_seq = not_error_body.dangling_instr_seq(Some(ValType::I32));
    let true_branch_id = true_branch_seq.id();
    let false_branch_seq = not_error_body.dangling_instr_seq(Some(ValType::I32));
    let false_branch_id = false_branch_seq.id();

    not_error_body.instr(walrus::ir::IfElse {
        consequent: true_branch_id,
        alternative: false_branch_id,
    });

    // True branch: evaluate and return true_value
    let mut true_body = not_error_body.instr_seq(true_branch_id);
    compile_expr(&call_expr.args[1].expr, &mut true_body, env, ctx, module)?;

    // False branch: evaluate and return false_value
    let mut false_body = not_error_body.instr_seq(false_branch_id);
    compile_expr(&call_expr.args[2].expr, &mut false_body, env, ctx, module)?;

    Ok(())
}

/// Compile the index operator: container[index]
fn compile_index(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 2 {
        anyhow::bail!("Index operator _[_] expects 2 arguments (container, index)");
    }
    // Compile container (array or map)
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    // Compile index (int for array, string for map)
    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
    // Call the polymorphic index function
    body.call(env.get(RuntimeFunction::ValueIndex));
    Ok(())
}

/// Compile the optional index operator: container[?index]
///
/// Returns `Optional(Some(value))` if the key/index exists,
/// `Optional(None)` if the key/index is absent (no error).
///
/// AST: `func_name = "_[?_]"`, `args = [container, index]`
fn compile_opt_index(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 2 {
        anyhow::bail!("Optional index operator _[?_] expects 2 arguments (container, index)");
    }
    // Compile container, then key; delegate all logic to the runtime
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    compile_expr(&call_expr.args[1].expr, body, env, ctx, module)?;
    body.call(env.get(RuntimeFunction::OptionalIndex));
    Ok(())
}

/// Compile the optional select operator: receiver?.field
///
/// Delegates to `cel_optional_select(receiver, field_name_ptr, field_name_len)` which
/// handles all cases: Optional(None), Optional(Some(map/object)), plain map, plain object.
///
/// AST: `func_name = "_?._"`, `args = [receiver, field_as_string_literal_or_ident]`
fn compile_opt_select(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    use cel::common::ast::Expr as CelExpr;

    if call_expr.args.len() != 2 {
        anyhow::bail!("Optional select operator _?._ expects 2 arguments");
    }

    // args[1] holds the field name.
    // The cel-rust parser emits it as a Literal(String) for `a?.b` (the field is quoted),
    // but other transformations may produce an Ident. Accept both.
    let field_name = match &call_expr.args[1].expr {
        CelExpr::Ident(name) => name.clone(),
        CelExpr::Literal(cel::common::ast::LiteralValue::String(s)) => s.to_string(),
        _ => anyhow::bail!("Optional select _?._ second argument must be a field name identifier"),
    };

    // Save receiver to a local so we can push arguments in the right order.
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    let receiver_local = module.locals.add(ValType::I32);
    body.local_set(receiver_local);

    // Write field name into Wasm memory, leaving (ptr, len) on the stack.
    let memory_id = helpers::get_memory_id(module)?;
    let field_ptr_local = module.locals.add(ValType::I32);
    helpers::emit_string_const(&field_name, body, env, memory_id, module);
    // emit_string_const leaves (ptr: i32, len: i32) on stack; store ptr for the call.
    let field_len_local = module.locals.add(ValType::I32);
    body.local_set(field_len_local);
    body.local_set(field_ptr_local);

    // Call cel_optional_select(receiver, field_ptr, field_len) → *mut CelValue
    body.local_get(receiver_local);
    body.local_get(field_ptr_local);
    body.local_get(field_len_local);
    body.call(env.get(RuntimeFunction::OptionalSelect));

    Ok(())
}

/// Top-level operator dispatcher. Returns `true` if the func_name was recognized as
/// an operator, `false` otherwise (so the caller can fall through to named functions).
pub fn compile_operator(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<bool, anyhow::Error> {
    match func_name {
        // Arithmetic operators
        operators::ADD => compile_binary_op(
            call_expr,
            "Add",
            RuntimeFunction::ValueAdd,
            body,
            env,
            ctx,
            module,
        )?,
        operators::SUBSTRACT => compile_binary_op(
            call_expr,
            "Subtract",
            RuntimeFunction::ValueSub,
            body,
            env,
            ctx,
            module,
        )?,
        operators::MULTIPLY => compile_binary_op(
            call_expr,
            "Multiply",
            RuntimeFunction::ValueMul,
            body,
            env,
            ctx,
            module,
        )?,
        operators::DIVIDE => compile_binary_op(
            call_expr,
            "Divide",
            RuntimeFunction::ValueDiv,
            body,
            env,
            ctx,
            module,
        )?,
        operators::MODULO => compile_binary_op(
            call_expr,
            "Modulo",
            RuntimeFunction::ValueMod,
            body,
            env,
            ctx,
            module,
        )?,
        operators::NEGATE => compile_unary_op(
            call_expr,
            "Negation",
            RuntimeFunction::ValueNegate,
            body,
            env,
            ctx,
            module,
        )?,

        // Comparison operators
        operators::EQUALS => compile_binary_op(
            call_expr,
            "Equals",
            RuntimeFunction::ValueEq,
            body,
            env,
            ctx,
            module,
        )?,
        operators::NOT_EQUALS => compile_binary_op(
            call_expr,
            "Not equals",
            RuntimeFunction::ValueNe,
            body,
            env,
            ctx,
            module,
        )?,
        operators::GREATER => compile_binary_op(
            call_expr,
            "Greater than",
            RuntimeFunction::ValueGt,
            body,
            env,
            ctx,
            module,
        )?,
        operators::LESS => compile_binary_op(
            call_expr,
            "Less than",
            RuntimeFunction::ValueLt,
            body,
            env,
            ctx,
            module,
        )?,
        operators::GREATER_EQUALS => compile_binary_op(
            call_expr,
            "Greater or equal",
            RuntimeFunction::ValueGte,
            body,
            env,
            ctx,
            module,
        )?,
        operators::LESS_EQUALS => compile_binary_op(
            call_expr,
            "Less or equal",
            RuntimeFunction::ValueLte,
            body,
            env,
            ctx,
            module,
        )?,

        // Membership operator
        operators::IN => compile_binary_op(
            call_expr,
            "'in'",
            RuntimeFunction::ValueIn,
            body,
            env,
            ctx,
            module,
        )?,

        // Logical operators
        operators::LOGICAL_AND => compile_logical_and(call_expr, body, env, ctx, module)?,
        operators::LOGICAL_OR => compile_logical_or(call_expr, body, env, ctx, module)?,
        operators::LOGICAL_NOT => compile_unary_op(
            call_expr,
            "Logical NOT",
            RuntimeFunction::BoolNot,
            body,
            env,
            ctx,
            module,
        )?,
        operators::NOT_STRICTLY_FALSE => compile_unary_op(
            call_expr,
            "@not_strictly_false",
            RuntimeFunction::NotStrictlyFalse,
            body,
            env,
            ctx,
            module,
        )?,

        // Conditional (ternary) operator
        operators::CONDITIONAL => compile_conditional(call_expr, body, env, ctx, module)?,

        // Index operator
        operators::INDEX => compile_index(call_expr, body, env, ctx, module)?,

        // Optional index operator: container[?key]
        operators::OPT_INDEX => compile_opt_index(call_expr, body, env, ctx, module)?,

        // Optional select operator: receiver?.field
        operators::OPT_SELECT => compile_opt_select(call_expr, body, env, ctx, module)?,

        // Not an operator
        _ => return Ok(false),
    }

    Ok(true)
}
