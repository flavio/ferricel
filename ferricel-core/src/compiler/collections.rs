use cel::common::ast::{ComprehensionExpr, EntryExpr, Expr, ListExpr, MapExpr, StructExpr};
use ferricel_types::functions::RuntimeFunction;
use slog::error;
use walrus::{InstrSeqBuilder, ValType};

use super::{
    access::resolve_type_name,
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::compile_string_to_local,
};

/// Compile an `Expr::List` node: `[elem1, elem2, ...]`
///
/// Handles optional elements `[?opt_expr, ...]`: if the element evaluates to
/// `Optional(Some(v))` it is unwrapped and appended; if `Optional(None)` it is skipped.
pub fn compile_list(
    list_expr: &ListExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    // Create an empty array
    body.call(env.get(RuntimeFunction::CreateArray));

    // Create a local to hold the array pointer while we push elements
    let array_ptr_local = module.locals.add(ValType::I32);
    body.local_set(array_ptr_local); // Pop array pointer into local

    // For each element in the list
    for (idx, element) in list_expr.elements.iter().enumerate() {
        let is_optional_elem = list_expr.optional_indices.contains(&idx);

        if is_optional_elem {
            // Compile the element expression — expected to return Optional
            compile_expr(&element.expr, body, env, ctx, module)?;
            let elem_local = module.locals.add(ValType::I32);
            body.local_tee(elem_local);

            // Check hasValue
            body.call(env.get(RuntimeFunction::OptionalHasValue));
            body.call(env.get(RuntimeFunction::ValueToBool));
            body.unop(walrus::ir::UnaryOp::I32WrapI64);

            // If has value: extract inner and push; else: skip
            let then_seq = body.dangling_instr_seq(None);
            let then_id = then_seq.id();
            let else_seq = body.dangling_instr_seq(None);
            let else_id = else_seq.id();

            body.instr(walrus::ir::IfElse {
                consequent: then_id,
                alternative: else_id,
            });

            // Then: extract and push
            {
                let mut then_body = body.instr_seq(then_id);
                then_body.local_get(elem_local);
                then_body.call(env.get(RuntimeFunction::OptionalValue));
                let inner_local = module.locals.add(ValType::I32);
                then_body.local_set(inner_local);
                then_body.local_get(array_ptr_local);
                then_body.local_get(inner_local);
                then_body.call(env.get(RuntimeFunction::ArrayPush));
            }

            // Else: no-op (empty block)
            // body.instr_seq(else_id) already empty
        } else {
            // Normal non-optional element
            compile_expr(&element.expr, body, env, ctx, module)?;

            let element_ptr_local = module.locals.add(ValType::I32);
            body.local_set(element_ptr_local);

            body.local_get(array_ptr_local);
            body.local_get(element_ptr_local);
            body.call(env.get(RuntimeFunction::ArrayPush));
        }
    }

    // Leave the array pointer on the stack
    body.local_get(array_ptr_local);
    Ok(())
}

/// Compile an `Expr::Map` node: `{"key": value, ...}`
///
/// Handles optional entries `{?key: opt_value}`: if the value evaluates to
/// `Optional(Some(v))` the entry is inserted with the unwrapped value;
/// if `Optional(None)` the entry is omitted entirely.
pub fn compile_map(
    map_expr: &MapExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    // Create an empty map
    body.call(env.get(RuntimeFunction::CreateMap));

    // Create a local to hold the map pointer while we insert entries
    let map_ptr_local = module.locals.add(ValType::I32);
    body.local_set(map_ptr_local); // Pop map pointer into local

    // For each entry in the map
    for entry in &map_expr.entries {
        match &entry.expr {
            EntryExpr::MapEntry(map_entry) => {
                if map_entry.optional {
                    // Optional entry: only insert if value is Optional(Some(...))
                    // Compile key
                    compile_expr(&map_entry.key.expr, body, env, ctx, module)?;
                    let key_ptr_local = module.locals.add(ValType::I32);
                    body.local_set(key_ptr_local);

                    // Compile value (expected to be Optional)
                    compile_expr(&map_entry.value.expr, body, env, ctx, module)?;
                    let value_opt_local = module.locals.add(ValType::I32);
                    body.local_tee(value_opt_local);

                    // Check hasValue
                    body.call(env.get(RuntimeFunction::OptionalHasValue));
                    body.call(env.get(RuntimeFunction::ValueToBool));
                    body.unop(walrus::ir::UnaryOp::I32WrapI64);

                    let then_seq = body.dangling_instr_seq(None);
                    let then_id = then_seq.id();
                    let else_seq = body.dangling_instr_seq(None);
                    let else_id = else_seq.id();

                    body.instr(walrus::ir::IfElse {
                        consequent: then_id,
                        alternative: else_id,
                    });

                    // Then: extract inner and insert
                    {
                        let mut then_body = body.instr_seq(then_id);
                        then_body.local_get(value_opt_local);
                        then_body.call(env.get(RuntimeFunction::OptionalValue));
                        let inner_local = module.locals.add(ValType::I32);
                        then_body.local_set(inner_local);
                        then_body.local_get(map_ptr_local);
                        then_body.local_get(key_ptr_local);
                        then_body.local_get(inner_local);
                        then_body.call(env.get(RuntimeFunction::MapInsert));
                    }

                    // Else: skip (empty block)
                } else {
                    // Normal non-optional entry
                    compile_expr(&map_entry.key.expr, body, env, ctx, module)?;
                    let key_ptr_local = module.locals.add(ValType::I32);
                    body.local_set(key_ptr_local);

                    compile_expr(&map_entry.value.expr, body, env, ctx, module)?;
                    let value_ptr_local = module.locals.add(ValType::I32);
                    body.local_set(value_ptr_local);

                    body.local_get(map_ptr_local);
                    body.local_get(key_ptr_local);
                    body.local_get(value_ptr_local);
                    body.call(env.get(RuntimeFunction::MapInsert));
                }
            }
            _ => anyhow::bail!("Unsupported map entry type: {:?}", entry.expr),
        }
    }

    // Leave the map pointer on the stack
    body.local_get(map_ptr_local);
    Ok(())
}

/// Compile an `Expr::Struct` node: `TypeName{field: value, ...}`
///
/// Structs are compiled as maps with string keys + a special `__type__` field.
pub fn compile_struct(
    struct_expr: &StructExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    // Resolve the type name using container-based resolution
    let resolved_type_name = resolve_type_name(&struct_expr.type_name, &ctx.container, &ctx.schema)
        .unwrap_or_else(|| struct_expr.type_name.clone());

    // Create an empty map to represent the struct
    body.call(env.get(RuntimeFunction::CreateMap));
    let map_ptr_local = module.locals.add(ValType::I32);
    body.local_set(map_ptr_local);

    // Insert "__type__" field to preserve type information
    // Use the resolved (fully-qualified) type name
    let type_key_local = compile_string_to_local("__type__", body, env, module)?;
    let type_value_local = compile_string_to_local(&resolved_type_name, body, env, module)?;

    body.local_get(map_ptr_local);
    body.local_get(type_key_local);
    body.local_get(type_value_local);
    body.call(env.get(RuntimeFunction::MapInsert));

    // If we have a schema, check if this type has wrapper fields and add metadata
    if let Some(schema) = &ctx.schema {
        // Check if the type exists in the schema (using resolved name)
        if resolved_type_name.contains('.') && !schema.has_message_type(&resolved_type_name) {
            // Warn if struct type looks like a proto message but is not in the schema
            error!(
                ctx.logger,
                "Struct '{}' looks like a protobuf message, but is not defined in the provided schema",
                struct_expr.type_name;
                "struct_type" => &struct_expr.type_name,
                "resolved_type" => &resolved_type_name
            );
            error!(ctx.logger, "Wrapper type semantics will not be available");
            error!(
                ctx.logger,
                "Ensure the correct proto descriptor files are provided with --proto-descriptor"
            );
        }

        let wrapper_fields = schema.get_wrapper_fields(&resolved_type_name);
        if !wrapper_fields.is_empty() {
            // Create an array of wrapper field names
            body.call(env.get(RuntimeFunction::CreateArray));
            let array_ptr_local = module.locals.add(ValType::I32);
            body.local_set(array_ptr_local);

            // Add each wrapper field name to the array
            for field_name in &wrapper_fields {
                let field_name_local = compile_string_to_local(field_name, body, env, module)?;
                body.local_get(array_ptr_local);
                body.local_get(field_name_local);
                body.call(env.get(RuntimeFunction::ArrayPush));
            }

            // Insert "__wrapper_fields__" metadata into the struct map
            let wrapper_key_local =
                compile_string_to_local("__wrapper_fields__", body, env, module)?;
            body.local_get(map_ptr_local);
            body.local_get(wrapper_key_local);
            body.local_get(array_ptr_local);
            body.call(env.get(RuntimeFunction::MapInsert));
        }

        // Emit "__field_defaults__" metadata: field_name → default_kind string.
        // This enables cel_get_field to return appropriate proto default values for
        // missing fields (e.g., empty map for map fields, empty list for repeated fields).
        let field_defaults = schema.get_field_default_kinds(&resolved_type_name);
        if !field_defaults.is_empty() {
            body.call(env.get(RuntimeFunction::CreateMap));
            let defaults_map_local = module.locals.add(ValType::I32);
            body.local_set(defaults_map_local);

            for (fname, fkind) in &field_defaults {
                let fname_local = compile_string_to_local(fname, body, env, module)?;
                let fkind_local = compile_string_to_local(fkind, body, env, module)?;
                body.local_get(defaults_map_local);
                body.local_get(fname_local);
                body.local_get(fkind_local);
                body.call(env.get(RuntimeFunction::MapInsert));
            }

            let defaults_key_local =
                compile_string_to_local("__field_defaults__", body, env, module)?;
            body.local_get(map_ptr_local);
            body.local_get(defaults_key_local);
            body.local_get(defaults_map_local);
            body.call(env.get(RuntimeFunction::MapInsert));
        }

        // If this is google.protobuf.Any, bake __any_schema__ from the type_url
        // so the runtime can do schema-aware wire comparison of Any.value bytes.
        if resolved_type_name == "google.protobuf.Any" {
            // Try to find a literal type_url in the struct entries at compile time
            use cel::common::ast::EntryExpr as EE;
            use cel::common::ast::LiteralValue;
            let maybe_type_url: Option<String> = struct_expr.entries.iter().find_map(|e| {
                if let EE::StructField(sf) = &e.expr
                    && sf.field == "type_url"
                    && let Expr::Literal(LiteralValue::String(s)) = &sf.value.expr
                {
                    return Some(s.inner().to_owned());
                }
                None
            });

            if let Some(type_url) = maybe_type_url {
                // Strip the well-known type.googleapis.com/ prefix
                let msg_type = if let Some(stripped) = type_url.strip_prefix("type.googleapis.com/")
                {
                    stripped.to_string()
                } else {
                    type_url.clone()
                };

                let field_schema = schema.get_any_field_schema(&msg_type);
                if !field_schema.is_empty() {
                    // Bake as a CelValue map: field_number_str → kind_str
                    body.call(env.get(RuntimeFunction::CreateMap));
                    let schema_map_local = module.locals.add(ValType::I32);
                    body.local_set(schema_map_local);

                    for (field_num, kind_str) in &field_schema {
                        let num_key_local =
                            compile_string_to_local(&field_num.to_string(), body, env, module)?;
                        let kind_val_local = compile_string_to_local(kind_str, body, env, module)?;
                        body.local_get(schema_map_local);
                        body.local_get(num_key_local);
                        body.local_get(kind_val_local);
                        body.call(env.get(RuntimeFunction::MapInsert));
                    }

                    // Also bake the resolved message type name as "__any_type__"
                    let any_type_key_local =
                        compile_string_to_local("__any_type__", body, env, module)?;
                    let any_type_val_local = compile_string_to_local(&msg_type, body, env, module)?;
                    body.local_get(schema_map_local);
                    body.local_get(any_type_key_local);
                    body.local_get(any_type_val_local);
                    body.call(env.get(RuntimeFunction::MapInsert));

                    // Insert "__any_schema__" into the Any struct map
                    let schema_key_local =
                        compile_string_to_local("__any_schema__", body, env, module)?;
                    body.local_get(map_ptr_local);
                    body.local_get(schema_key_local);
                    body.local_get(schema_map_local);
                    body.call(env.get(RuntimeFunction::MapInsert));
                }
            }
        }
    } else if resolved_type_name.contains('.') {
        // Warn if struct type looks like a proto message but no schema provided
        error!(
            ctx.logger,
            "Struct '{}' looks like a protobuf message, but no schema provided",
            struct_expr.type_name;
            "struct_type" => &struct_expr.type_name,
            "resolved_type" => &resolved_type_name
        );
        error!(ctx.logger, "Wrapper type semantics will not be available");
        error!(ctx.logger, "Use --proto-descriptor to provide schema");
    }

    // Insert each struct field
    for entry in &struct_expr.entries {
        match &entry.expr {
            EntryExpr::StructField(struct_field) => {
                if struct_field.optional {
                    // Optional field: `{?field: opt_expr}` — only insert if opt_expr is Optional(Some)
                    // Pre-compute the field key string into a local (before the branch)
                    let field_key_local =
                        compile_string_to_local(&struct_field.field, body, env, module)?;

                    // Compile the value (expected to be Optional)
                    compile_expr(&struct_field.value.expr, body, env, ctx, module)?;
                    let opt_local = module.locals.add(ValType::I32);
                    body.local_tee(opt_local);

                    // Check hasValue
                    body.call(env.get(RuntimeFunction::OptionalHasValue));
                    body.call(env.get(RuntimeFunction::ValueToBool));
                    body.unop(walrus::ir::UnaryOp::I32WrapI64);

                    let then_seq = body.dangling_instr_seq(None);
                    let then_id = then_seq.id();
                    let else_seq = body.dangling_instr_seq(None);
                    let else_id = else_seq.id();

                    body.instr(walrus::ir::IfElse {
                        consequent: then_id,
                        alternative: else_id,
                    });

                    // Then branch: unwrap and insert into struct
                    {
                        let mut then_body = body.instr_seq(then_id);
                        then_body.local_get(opt_local);
                        then_body.call(env.get(RuntimeFunction::OptionalValue));
                        let field_value_local = module.locals.add(ValType::I32);
                        then_body.local_set(field_value_local);

                        then_body.local_get(map_ptr_local);
                        then_body.local_get(field_key_local);
                        then_body.local_get(field_value_local);
                        then_body.call(env.get(RuntimeFunction::MapInsert));
                    }

                    // Else branch: skip (do nothing)
                    {
                        body.instr_seq(else_id);
                    }
                } else {
                    // Normal (non-optional) field
                    let field_key_local =
                        compile_string_to_local(&struct_field.field, body, env, module)?;

                    compile_expr(&struct_field.value.expr, body, env, ctx, module)?;
                    let field_value_local = module.locals.add(ValType::I32);
                    body.local_set(field_value_local);

                    body.local_get(map_ptr_local);
                    body.local_get(field_key_local);
                    body.local_get(field_value_local);
                    body.call(env.get(RuntimeFunction::MapInsert));
                }
            }
            _ => anyhow::bail!("Unsupported struct entry type: {:?}", entry.expr),
        }
    }

    // Leave the map pointer on the stack
    body.local_get(map_ptr_local);
    Ok(())
}

/// Compile an `Expr::Comprehension` node: macros like `all()`, `exists()`, `filter()`, etc.
///
/// The CEL parser automatically expands these macros into comprehension expressions.
///
/// This implementation supports both list and map ranges via `cel_iter_prepare`:
/// - Lists: iterates elements directly (index 0..len).
/// - Maps: `cel_iter_prepare` extracts the keys into an array; the iter_var is bound to each key.
///
/// # Map iteration (single-variable)
///
/// For single-variable comprehensions over maps, the iter_var is bound to the **key** at each
/// step. This matches the CEL specification: `m.exists(k, pred)`, `m.all(k, pred)`,
/// `m.map(k, f)`, and `m.filter(k, pred)` all iterate over the map's keys.
///
/// # Map/filter error propagation
///
/// The `map` and `filter` macros expand to a list-concat loop_step (`accu + [expr]`). If the
/// transform or predicate expression produces a `CelValue::Error`, this implementation detects
/// the pattern and propagates the error immediately rather than embedding it in the result list.
pub fn compile_comprehension(
    comp_expr: &ComprehensionExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    // Step 1: Compile the iter_range (the list or map to iterate over)
    compile_expr(&comp_expr.iter_range.expr, body, env, ctx, module)?;

    // Store the range pointer (original; used for element access)
    let range_local = module.locals.add(ValType::I32);
    body.local_set(range_local);

    // Step 2: Prepare for iteration — for lists returns range itself; for maps returns keys array.
    body.local_get(range_local);
    body.call(env.get(RuntimeFunction::IterPrepare));
    let prepared_local = module.locals.add(ValType::I32);
    body.local_set(prepared_local);

    // Step 3: Get the iteration length from the prepared array
    body.local_get(prepared_local);
    body.call(env.get(RuntimeFunction::ArrayLen)); // Returns i32 length
    let length_local = module.locals.add(ValType::I32);
    body.local_set(length_local);

    // Step 4: Initialize the accumulator variable
    compile_expr(&comp_expr.accu_init.expr, body, env, ctx, module)?;
    let accu_local = module.locals.add(ValType::I32);
    body.local_set(accu_local);

    // Step 5: Initialize loop counter (index = 0)
    let index_local = module.locals.add(ValType::I32);
    body.i32_const(0);
    body.local_set(index_local);

    // Step 6: Create the loop using Wasm block/loop instructions
    // Structure: block $exit { loop $continue { ... } }
    let exit_block = body.dangling_instr_seq(None);
    let exit_block_id = exit_block.id();
    let continue_loop = body.dangling_instr_seq(None);
    let continue_loop_id = continue_loop.id();

    body.instr(walrus::ir::Block { seq: exit_block_id });

    // Start of exit block
    body.instr_seq(exit_block_id).instr(walrus::ir::Loop {
        seq: continue_loop_id,
    });

    // Start of continue loop
    let mut loop_body = body.instr_seq(continue_loop_id);

    // Check if index >= length (exit condition)
    loop_body.local_get(index_local);
    loop_body.local_get(length_local);
    loop_body.binop(walrus::ir::BinaryOp::I32GeU); // index >= length?
    loop_body.instr(walrus::ir::BrIf {
        block: exit_block_id,
    }); // Exit if true

    // Get the current element/key using ArrayGet(prepared, index).
    //
    // For lists:  prepared = the list itself  → ArrayGet returns the element at that index.
    // For maps:   prepared = the keys array   → ArrayGet returns the key at that index.
    //
    // This correctly implements single-var CEL semantics:
    //   - list.exists(e, pred)  → e = each element
    //   - map.exists(k, pred)   → k = each key     (NOT the value)
    //
    // Previously `cel_iter_var2` was used here, which returns the map VALUE for maps —
    // that violated the CEL spec which requires single-var map iteration over keys.
    loop_body.local_get(prepared_local);
    loop_body.local_get(index_local);
    loop_body.call(env.get(RuntimeFunction::ArrayGet));
    let element_local = module.locals.add(ValType::I32);
    loop_body.local_set(element_local);

    // Create a new context with the iteration variable bound to the element/key
    let inner_ctx = ctx.with_local(comp_expr.iter_var.clone(), element_local);

    // Also bind the accumulator variable to the context
    let inner_ctx = inner_ctx.with_local(comp_expr.accu_var.clone(), accu_local);

    // Detect the map/filter pattern: accu_init is an empty list and loop_step is a list concat.
    // In this case we compile the step with explicit error propagation instead of using the
    // generic compile_expr path (which would embed errors into the result list).
    let is_list_accumulator =
        matches!(&comp_expr.accu_init.expr, Expr::List(l) if l.elements.is_empty());
    let map_inner = if is_list_accumulator {
        extract_map_concat_inner(&comp_expr.loop_step.expr, &comp_expr.accu_var)
    } else {
        None
    };

    if let Some(inner_expr) = map_inner {
        // Map/filter pattern: compile inner_expr, check for error, then push to accu.
        // If inner_expr errors, store error in accu and exit the loop.
        let result_local = module.locals.add(ValType::I32);
        compile_expr(inner_expr, &mut loop_body, env, &inner_ctx, module)?;
        loop_body.local_set(result_local);
        loop_body.local_get(result_local);
        loop_body.call(env.get(RuntimeFunction::IsError));

        let err_then = loop_body.dangling_instr_seq(None);
        let err_then_id = err_then.id();
        let err_else = loop_body.dangling_instr_seq(None);
        let err_else_id = err_else.id();
        {
            let mut t = loop_body.instr_seq(err_then_id);
            t.local_get(result_local);
            t.local_set(accu_local);
            t.instr(walrus::ir::Br {
                block: exit_block_id,
            });
        }
        {
            let mut e = loop_body.instr_seq(err_else_id);
            e.local_get(accu_local);
            e.local_get(result_local);
            e.call(env.get(RuntimeFunction::ArrayPush));
        }
        loop_body.instr(walrus::ir::IfElse {
            consequent: err_then_id,
            alternative: err_else_id,
        });
    } else {
        // General case: compile the loop_step normally.
        compile_expr(
            &comp_expr.loop_step.expr,
            &mut loop_body,
            env,
            &inner_ctx,
            module,
        )?;
        // Store the new accumulator value
        loop_body.local_set(accu_local);
    }

    // Check the loop_cond to see if we should short-circuit
    // For all(), this is: @not_strictly_false(@result)
    // Re-bind accu in inner_ctx since it may have been updated
    let inner_ctx = inner_ctx.with_local(comp_expr.accu_var.clone(), accu_local);
    compile_expr(
        &comp_expr.loop_cond.expr,
        &mut loop_body,
        env,
        &inner_ctx,
        module,
    )?;

    // Convert the loop condition (CelValue::Bool) to i64 (0 or 1)
    loop_body.call(env.get(RuntimeFunction::ValueToBool)); // Returns i64: 1 if true, 0 if false

    // If the condition is false (0), we should exit the loop
    // i32.eqz checks if value is 0, returns 1 if yes, 0 if no
    loop_body.unop(walrus::ir::UnaryOp::I64Eqz); // 1 if cond was false, 0 if true
    loop_body.instr(walrus::ir::BrIf {
        block: exit_block_id,
    }); // Exit if condition was false

    // Increment the index
    loop_body.local_get(index_local);
    loop_body.i32_const(1);
    loop_body.binop(walrus::ir::BinaryOp::I32Add);
    loop_body.local_set(index_local);

    // Continue the loop
    loop_body.instr(walrus::ir::Br {
        block: continue_loop_id,
    }); // Jump back to start of loop

    // After the loop, compile the result expression
    // For all(), this is just @result (the accumulator)
    let result_ctx = ctx.with_local(comp_expr.accu_var.clone(), accu_local);
    compile_expr(&comp_expr.result.expr, body, env, &result_ctx, module)?;

    Ok(())
}

/// Detect the `map` comprehension loop_step pattern.
///
/// The CEL parser expands `list.map(n, f(n))` to a comprehension whose `loop_step` is:
///   `_+_(@accu, [f(n)])`
///
/// For `filter`, the step is conditional:
///   `@accu_var ? _+_(@accu, [n]) : @accu`  (i.e. `_?_(_:_)(pred, _+_(@accu, [elem]), @accu)`)
///
/// In both cases we want to find the inner expression being appended so we can check it for
/// errors before pushing. Returns the inner expression if the pattern matches, `None` otherwise.
fn extract_map_concat_inner<'a>(step: &'a Expr, accu_var: &str) -> Option<&'a Expr> {
    match step {
        // Direct concat: _+_(@accu, [inner])
        Expr::Call(call) if call.func_name == "_+_" && call.args.len() == 2 => {
            let rhs = &call.args[1].expr;
            if let Expr::List(list) = rhs
                && list.elements.len() == 1
                && !list.optional_indices.contains(&0)
            {
                return Some(&list.elements[0].expr);
            }
            None
        }
        // Filter conditional: _?_(_:_)(pred, _+_(@accu, [elem]), @accu_ref)
        Expr::Call(call) if call.func_name == "_?_(_:_)" && call.args.len() == 3 => {
            // args[1] should be _+_(@accu, [elem])
            let consequent = &call.args[1].expr;
            // args[2] should be @accu (the else branch keeps accu unchanged)
            let alternative = &call.args[2].expr;
            let alt_is_accu = matches!(alternative, Expr::Ident(name) if name == accu_var);
            if alt_is_accu {
                extract_map_concat_inner(consequent, accu_var)
            } else {
                None
            }
        }
        _ => None,
    }
}
