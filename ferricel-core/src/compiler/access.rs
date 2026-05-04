use cel::common::ast::Expr;
use ferricel_types::functions::RuntimeFunction;
use walrus::{InstrSeqBuilder, ValType};

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    helpers::{emit_string_const, get_memory_id},
};

/// Resolves a type name using container-based hierarchical resolution.
///
/// This implements the CEL container resolution algorithm:
/// For a name like "MyType" with container "A.B.C", tries in order:
/// 1. A.B.C.MyType (most specific)
/// 2. A.B.MyType (parent level)
/// 3. A.MyType (grandparent level)
/// 4. MyType (root level)
///
/// Special case: Leading dot (.MyType) bypasses container and only tries root level.
///
/// Returns the resolved fully-qualified type name, or None if not found.
pub fn resolve_type_name(
    name: &str,
    container: &Option<String>,
    schema: &Option<crate::schema::ProtoSchema>,
) -> Option<String> {
    // Early return if no schema - can't resolve anything
    let schema = schema.as_ref()?;

    // Special case: Leading dot means root scope only
    // ".MyType" -> try only "MyType" (bypass container)
    if let Some(stripped) = name.strip_prefix('.') {
        return if schema.has_message_type(stripped) {
            Some(stripped.to_string())
        } else {
            None
        };
    }

    // Try exact name first (optimization for fully-qualified names)
    if schema.has_message_type(name) {
        return Some(name.to_string());
    }

    // Try container-based hierarchical resolution
    if let Some(container_str) = container {
        let parts: Vec<&str> = container_str.split('.').collect();

        // Try from most specific to least: A.B.C.name -> A.B.name -> A.name -> name
        for i in (1..=parts.len()).rev() {
            let prefix = parts[0..i].join(".");
            let qualified = format!("{}.{}", prefix, name);

            if schema.has_message_type(&qualified) {
                return Some(qualified);
            }
        }
    }

    // Final attempt: try name at root level again
    // (This handles the case where container is set but name is at root)
    if schema.has_message_type(name) {
        return Some(name.to_string());
    }

    None
}

/// Build the ordered list of candidate variable names for a given base name and container.
///
/// CEL name resolution order (most specific to least):
///   container.A.B.C.name, ..., A.name, name
///
/// If the name has a leading dot, only the root-scope name is returned (container bypassed).
pub fn variable_candidates(name: &str, container: &Option<String>) -> Vec<String> {
    // Leading dot means root scope only — bypass container
    if let Some(stripped) = name.strip_prefix('.') {
        return vec![stripped.to_string()];
    }

    let mut candidates = Vec::new();

    if let Some(container_str) = container {
        let parts: Vec<&str> = container_str.split('.').collect();
        // Most specific first: A.B.C.name, A.B.name, A.name
        for i in (1..=parts.len()).rev() {
            let prefix = parts[0..i].join(".");
            candidates.push(format!("{}.{}", prefix, name));
        }
    }

    // Always try the bare name last (root scope)
    candidates.push(name.to_string());

    candidates
}

/// Emit Wasm code that tries each candidate variable name in order, returning the
/// first non-null result. Leaves a `*mut CelValue` (i32) on the stack.
///
/// If none of the candidates resolves, the result is a **null pointer**. Use this
/// only when the caller needs the null as a "not found" sentinel (e.g. `compile_select`
/// which falls back to field access). For `compile_ident`, use
/// `emit_variable_lookup_chain` which converts null to a runtime error.
fn emit_variable_lookup_chain_raw(
    candidates: &[String],
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    module: &mut walrus::Module,
) -> Result<walrus::LocalId, anyhow::Error> {
    let memory_id = get_memory_id(module)?;
    let result_local = module.locals.add(ValType::I32);

    // Helper: emit `cel_get_variable(name)` and store into result_local.
    let emit_get = |name: &str,
                    body: &mut InstrSeqBuilder,
                    module: &mut walrus::Module|
     -> Result<(), anyhow::Error> {
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as i32;
        let ptr_local = module.locals.add(ValType::I32);

        body.i32_const(name_len)
            .call(env.get(RuntimeFunction::Malloc))
            .local_set(ptr_local);

        for (offset, &byte) in name_bytes.iter().enumerate() {
            body.local_get(ptr_local);
            body.i32_const(byte as i32);
            body.store(
                memory_id,
                walrus::ir::StoreKind::I32_8 { atomic: false },
                walrus::ir::MemArg {
                    align: 1,
                    offset: offset as u64,
                },
            );
        }

        body.local_get(ptr_local)
            .i32_const(name_len)
            .call(env.get(RuntimeFunction::GetVariable))
            .local_set(result_local);

        Ok(())
    };

    if candidates.is_empty() {
        body.i32_const(0).local_set(result_local);
        body.local_get(result_local);
        return Ok(result_local);
    }

    if candidates.len() == 1 {
        emit_get(&candidates[0], body, module)?;
        body.local_get(result_local);
        return Ok(result_local);
    }

    emit_get(&candidates[0], body, module)?;
    build_fallback_chain(&candidates[1..], result_local, body, env, module, memory_id)?;

    body.local_get(result_local);
    Ok(result_local)
}

/// Emit `cel_unbound_variable_error(name_ptr, name_len)` and store result into `result_local`.
fn emit_unbound_error(
    name: &str,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    module: &mut walrus::Module,
    memory_id: walrus::MemoryId,
) -> Result<walrus::LocalId, anyhow::Error> {
    let result_local = module.locals.add(ValType::I32);
    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len() as i32;
    let ptr_local = module.locals.add(ValType::I32);

    body.i32_const(name_len)
        .call(env.get(RuntimeFunction::Malloc))
        .local_set(ptr_local);

    for (offset, &byte) in name_bytes.iter().enumerate() {
        body.local_get(ptr_local);
        body.i32_const(byte as i32);
        body.store(
            memory_id,
            walrus::ir::StoreKind::I32_8 { atomic: false },
            walrus::ir::MemArg {
                align: 1,
                offset: offset as u64,
            },
        );
    }

    body.local_get(ptr_local)
        .i32_const(name_len)
        .call(env.get(RuntimeFunction::UnboundVariableError))
        .local_set(result_local);

    Ok(result_local)
}

/// After the lookup chain, if `result_local` is still null emit `cel_unbound_variable_error`
/// and store its result back into `result_local`.
fn emit_null_to_unbound_error(
    name: &str,
    result_local: walrus::LocalId,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    module: &mut walrus::Module,
    memory_id: walrus::MemoryId,
) -> Result<(), anyhow::Error> {
    body.local_get(result_local)
        .unop(walrus::ir::UnaryOp::I32Eqz);

    let then_seq = body.dangling_instr_seq(None);
    let then_id = then_seq.id();
    let else_seq = body.dangling_instr_seq(None);
    let else_id = else_seq.id();

    body.instr(walrus::ir::IfElse {
        consequent: then_id,
        alternative: else_id,
    });

    // Then branch (null): emit error and store into result_local
    {
        let mut then_body = body.instr_seq(then_id);
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as i32;
        let ptr_local = module.locals.add(ValType::I32);

        then_body
            .i32_const(name_len)
            .call(env.get(RuntimeFunction::Malloc))
            .local_set(ptr_local);

        for (offset, &byte) in name_bytes.iter().enumerate() {
            then_body.local_get(ptr_local);
            then_body.i32_const(byte as i32);
            then_body.store(
                memory_id,
                walrus::ir::StoreKind::I32_8 { atomic: false },
                walrus::ir::MemArg {
                    align: 1,
                    offset: offset as u64,
                },
            );
        }

        then_body
            .local_get(ptr_local)
            .i32_const(name_len)
            .call(env.get(RuntimeFunction::UnboundVariableError))
            .local_set(result_local);
    }

    // Else branch: result already valid, nothing to do

    Ok(())
}

/// Emit Wasm code that tries each candidate variable name in order, returning the
/// first non-null result. If no candidate resolves, emits a call to
/// `cel_unbound_variable_error` with the bare (last) candidate name so the result
/// is always a valid (non-null) `*mut CelValue`. Leaves an i32 on the stack.
///
/// Callers that need a null sentinel for "not found" (e.g. `compile_select`) must
/// use `emit_variable_lookup_chain_raw` instead.
fn emit_variable_lookup_chain(
    candidates: &[String],
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    module: &mut walrus::Module,
) -> Result<walrus::LocalId, anyhow::Error> {
    let memory_id = get_memory_id(module)?;

    if candidates.is_empty() {
        // No candidates at all — emit an error directly with an empty name
        let result_local = emit_unbound_error("", body, env, module, memory_id)?;
        body.local_get(result_local);
        return Ok(result_local);
    }

    // Run the raw lookup chain (may leave null in result_local on miss)
    let result_local = emit_variable_lookup_chain_raw(candidates, body, env, module)?;
    // Pop the value left on the stack by the raw chain — we'll put it back after the check
    body.local_set(result_local);

    // If null, replace with a proper error value naming the variable
    let bare_name = candidates.last().unwrap().as_str();
    emit_null_to_unbound_error(bare_name, result_local, body, env, module, memory_id)?;

    body.local_get(result_local);
    Ok(result_local)
}

/// Recursively builds the "if null, try next" chain for a slice of candidate names.
fn build_fallback_chain(
    remaining: &[String],
    result_local: walrus::LocalId,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    module: &mut walrus::Module,
    memory_id: walrus::MemoryId,
) -> Result<(), anyhow::Error> {
    if remaining.is_empty() {
        return Ok(());
    }

    // if result_local == 0 (null): try next candidate
    body.local_get(result_local)
        .unop(walrus::ir::UnaryOp::I32Eqz);

    let then_seq = body.dangling_instr_seq(None);
    let then_id = then_seq.id();
    let else_seq = body.dangling_instr_seq(None);
    let else_id = else_seq.id();

    body.instr(walrus::ir::IfElse {
        consequent: then_id,
        alternative: else_id,
    });

    // Then branch: try next candidate
    {
        let mut then_body = body.instr_seq(then_id);
        let name = &remaining[0];
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as i32;
        let ptr_local = module.locals.add(ValType::I32);

        then_body
            .i32_const(name_len)
            .call(env.get(RuntimeFunction::Malloc))
            .local_set(ptr_local);

        for (offset, &byte) in name_bytes.iter().enumerate() {
            then_body.local_get(ptr_local);
            then_body.i32_const(byte as i32);
            then_body.store(
                memory_id,
                walrus::ir::StoreKind::I32_8 { atomic: false },
                walrus::ir::MemArg {
                    align: 1,
                    offset: offset as u64,
                },
            );
        }

        then_body
            .local_get(ptr_local)
            .i32_const(name_len)
            .call(env.get(RuntimeFunction::GetVariable))
            .local_set(result_local);

        build_fallback_chain(
            &remaining[1..],
            result_local,
            &mut then_body,
            env,
            module,
            memory_id,
        )?;
    }

    // Else branch: result is already set (non-null), nothing to do
    // (empty else branch is valid in Wasm)

    Ok(())
}

/// Attempt to collect a `Select` (or `Ident`) chain into a dotted qualified name.
///
/// Returns `Some("a.b.c")` when the entire expression is of the form
/// `Ident("a") -> select "b" -> select "c"` with no `has()` test nodes.
/// Returns `None` as soon as any node is not a simple ident or a non-test select,
/// which means the expression represents a real runtime field access.
pub fn try_collect_qualified_ident(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(name) => Some(name.clone()),
        Expr::Select(s) if !s.test => {
            let base = try_collect_qualified_ident(&s.operand.expr)?;
            Some(format!("{}.{}", base, s.field))
        }
        _ => None,
    }
}

/// Return the root identifier name of a Select chain, or the ident name itself.
/// e.g. `x.y.z` → `Some("x")`, `x` → `Some("x")`, `f(x).y` → `None`
fn root_ident(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name) => Some(name.as_str()),
        Expr::Select(s) if !s.test => root_ident(&s.operand.expr),
        _ => None,
    }
}

/// Compile an `Expr::Ident` node.
///
/// Resolution order (per CEL spec):
/// 1. Local variables from comprehension scope (fast path via local_get)
/// 2. Type denotations (bool, int, uint, double, string, bytes, list, map, etc.)
/// 3. Runtime variable lookup with container-aware resolution:
///    - With container "A.B": tries A.B.name, A.name, name (first non-null wins)
///    - Without container: tries name directly
pub fn compile_ident(
    name: &str,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    // First check if this is a local variable (from comprehension scope)
    if let Some(&local_id) = ctx.local_vars.get(name) {
        body.local_get(local_id);
        return Ok(());
    }

    // Type denotations - these are constant Type values
    // Note: "dyn" is NOT a type denotation - it's only valid as a function call
    match name {
        "bool" | "int" | "uint" | "double" | "string" | "bytes" | "list" | "map" | "null_type"
        | "type" | "timestamp" | "duration" | "optional_type" => {
            let memory_id = get_memory_id(module)?;
            emit_string_const(name, body, env, memory_id, module);
            body.call(env.get(RuntimeFunction::CreateType));
        }
        _ => {
            // Build the ordered list of candidates using container resolution
            let candidates = variable_candidates(name, &ctx.container);
            emit_variable_lookup_chain(&candidates, body, env, module)?;
        }
    }

    Ok(())
}

/// Compile an `Expr::Select` node.
///
/// Resolution order (per CEL spec — longest prefix wins):
/// 1. If the entire chain is a known proto type or K8s type literal → type denotation
/// 2. If the root ident is a comprehension local → skip variable lookup, do field access
/// 3. Try to resolve the full dotted name as a variable (with container expansion):
///    - e.g. `x.y` with container `A.B` tries: `A.B.x.y`, `A.x.y`, `x.y`
///    - If found, use it
/// 4. Fall back to field access: resolve the operand (recursively applying same rules),
///    then call cel_get_field
pub fn compile_select(
    select_expr: &cel::common::ast::SelectExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    // Well-known Kubernetes CEL extension type literals.
    const K8S_TYPE_LITERALS: &[&str] = &["net.IP", "net.CIDR"];

    // Check whether the entire Select chain forms a qualified type name.
    let qualified_name = if !select_expr.test {
        try_collect_qualified_ident(&Expr::Select(select_expr.clone())).filter(|name| {
            K8S_TYPE_LITERALS.contains(&name.as_str())
                || ctx
                    .schema
                    .as_ref()
                    .map(|s| s.has_message_type(name))
                    .unwrap_or(false)
        })
    } else {
        None
    };

    if let Some(type_name) = qualified_name {
        // Type denotation: cel_create_type(ptr, len)
        let memory_id = get_memory_id(module)?;
        emit_string_const(&type_name, body, env, memory_id, module);
        body.call(env.get(RuntimeFunction::CreateType));
        return Ok(());
    }

    // If the root of this Select chain is a comprehension local, skip variable
    // lookup entirely — it must be a field access on the local value.
    let root_is_local = root_ident(&Expr::Select(select_expr.clone()))
        .map(|r| ctx.local_vars.contains_key(r))
        .unwrap_or(false);

    if !select_expr.test && !root_is_local {
        // Try to resolve the full dotted name as a variable first (longest prefix rule).
        if let Some(full_name) = try_collect_qualified_ident(&Expr::Select(select_expr.clone())) {
            let candidates = variable_candidates(&full_name, &ctx.container);

            // Emit: result = try_variable_chain(candidates)
            // If non-null, use it. If null, fall through to field access.
            let memory_id = get_memory_id(module)?;
            let result_local = module.locals.add(ValType::I32);

            // Try all candidates (raw — returns null if not found, for field-access fallback)
            emit_variable_lookup_chain_raw(&candidates, body, env, module)?;
            body.local_set(result_local);

            // if result != null: use it; else: do field access
            body.local_get(result_local)
                .unop(walrus::ir::UnaryOp::I32Eqz);

            let then_seq = body.dangling_instr_seq(Some(ValType::I32));
            let then_id = then_seq.id();
            let else_seq = body.dangling_instr_seq(Some(ValType::I32));
            let else_id = else_seq.id();

            body.instr(walrus::ir::IfElse {
                consequent: then_id,
                alternative: else_id,
            });

            // Then branch (null — not found as variable): do field access
            {
                let mut then_body = body.instr_seq(then_id);
                compile_field_access(select_expr, &mut then_body, env, ctx, module, memory_id)?;
            }

            // Else branch (non-null — found as variable): return result
            body.instr_seq(else_id).local_get(result_local);

            return Ok(());
        }
    }

    // Default: plain field access (test selects, local-rooted chains, non-collectable chains)
    let memory_id = get_memory_id(module)?;
    compile_field_access(select_expr, body, env, ctx, module, memory_id)?;

    Ok(())
}

/// Emit field-access Wasm for a SelectExpr: compile the operand, then call GetField/HasField.
fn compile_field_access(
    select_expr: &cel::common::ast::SelectExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
    memory_id: walrus::MemoryId,
) -> Result<(), anyhow::Error> {
    super::expr::compile_expr(&select_expr.operand.expr, body, env, ctx, module)?;

    let field_name = &select_expr.field;
    let field_bytes = field_name.as_bytes();
    let field_len = field_bytes.len() as i32;

    let field_ptr_local = module.locals.add(ValType::I32);

    body.i32_const(field_len)
        .call(env.get(RuntimeFunction::Malloc))
        .local_tee(field_ptr_local);

    for (offset, &byte) in field_bytes.iter().enumerate() {
        body.local_get(field_ptr_local);
        body.i32_const(byte as i32);
        body.store(
            memory_id,
            walrus::ir::StoreKind::I32_8 { atomic: false },
            walrus::ir::MemArg {
                align: 1,
                offset: offset as u64,
            },
        );
    }

    body.i32_const(field_len);

    if select_expr.test {
        body.call(env.get(RuntimeFunction::HasField));
    } else {
        body.call(env.get(RuntimeFunction::GetField));
    }

    Ok(())
}
