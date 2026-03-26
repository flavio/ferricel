use cel::common::ast::Expr;
use ferricel_types::functions::RuntimeFunction;
use walrus::{InstrSeqBuilder, ValType};

use super::{
    context::{CompilerContext, CompilerEnv},
    helpers::{compile_string_to_local, get_memory_id},
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

/// Compile an `Expr::Ident` node.
///
/// Handles:
/// - Local variables from comprehension scope (fast path via local_get)
/// - Type denotations (bool, int, uint, double, string, bytes, list, map, etc.)
/// - Runtime variables looked up from the bindings map
pub fn compile_ident(
    name: &str,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    // First check if this is a local variable (from comprehension scope)
    if let Some(&local_id) = ctx.local_vars.get(name) {
        // This is a local variable, load it from the local
        body.local_get(local_id);
        return Ok(());
    }

    // Not a local variable, check global variables and type denotations
    // Type denotations - these are constant Type values
    // Note: "dyn" is NOT a type denotation - it's only valid as a function call
    match name {
        "bool" | "int" | "uint" | "double" | "string" | "bytes" | "list" | "map" | "null_type"
        | "type" | "timestamp" | "duration" => {
            // Create a Type value for this type denotation
            let type_name_local = compile_string_to_local(name, body, env, module)?;
            body.local_get(type_name_local)
                .call(env.get(RuntimeFunction::CreateType));
        }
        _ => {
            // All other identifiers are runtime variables
            // Look them up from the bindings map via cel_get_variable
            let var_name = name.as_bytes();
            let var_name_len = var_name.len() as i32;

            // Allocate memory for variable name
            let var_name_ptr_local = module.locals.add(ValType::I32);
            body.i32_const(var_name_len)
                .call(env.get(RuntimeFunction::Malloc))
                .local_set(var_name_ptr_local);

            // Write variable name bytes to memory
            let memory_id = get_memory_id(module)?;

            for (offset, &byte) in var_name.iter().enumerate() {
                body.local_get(var_name_ptr_local);
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

            // Call cel_get_variable(name_ptr, name_len) -> *mut CelValue
            body.local_get(var_name_ptr_local)
                .i32_const(var_name_len)
                .call(env.get(RuntimeFunction::GetVariable));

            // The result is a pointer to the variable's value (or null if not found)
            // Null will cause runtime errors when operations try to use it
        }
    }

    Ok(())
}

/// Compile an `Expr::Select` node.
///
/// Handles:
/// - Qualified type names that map to proto message types (compiled as type denotations)
/// - Regular field access: compiles operand, then calls GetField or HasField
pub fn compile_select(
    select_expr: &cel::common::ast::SelectExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    // Before treating this as a runtime field access, check whether the entire
    // Select chain forms a qualified type name that exists in the proto schema.
    let qualified_name = if !select_expr.test {
        try_collect_qualified_ident(&Expr::Select(select_expr.clone())).filter(|name| {
            ctx.schema
                .as_ref()
                .map(|s| s.has_message_type(name))
                .unwrap_or(false)
        })
    } else {
        None
    };

    if let Some(type_name) = qualified_name {
        // Emit a type denotation: cel_create_type(ptr, len)
        let type_name_local = compile_string_to_local(&type_name, body, env, module)?;
        body.local_get(type_name_local)
            .call(env.get(RuntimeFunction::CreateType));
    } else {
        // Regular field access: compile operand, then call cel_get_field / cel_has_field
        super::expr::compile_expr(&select_expr.operand.expr, body, env, ctx, module)?;

        let field_name = &select_expr.field;
        let field_bytes = field_name.as_bytes();
        let field_len = field_bytes.len() as i32;

        let field_ptr_local = module.locals.add(ValType::I32);

        body.i32_const(field_len)
            .call(env.get(RuntimeFunction::Malloc))
            .local_tee(field_ptr_local);

        let memory_id = get_memory_id(module)?;

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
    }

    Ok(())
}
