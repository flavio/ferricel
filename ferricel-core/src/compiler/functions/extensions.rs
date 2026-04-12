use cel::common::ast::{CallExpr, Expr};
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CallShape, CompilerContext, CompilerEnv, ExtensionKey},
    expr::compile_expr,
    helpers::{emit_string_const, get_memory_id},
};

/// Emit a WASM runtime error value for an unknown function call.
///
/// Instead of failing at compile time, this emits instructions that produce a
/// `CelValue::Error("no matching overload")` at runtime — allowing the CEL
/// short-circuit operators (`||`, `&&`) to handle the error gracefully.
pub fn emit_unknown_function_error(
    func_name: &str,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let msg = format!("no matching overload for '{}'", func_name);
    let msg_bytes = msg.as_bytes();
    let msg_len = msg_bytes.len() as i32;
    let memory_id = get_memory_id(module)?;
    let ptr_local = module.locals.add(walrus::ValType::I32);
    body.i32_const(msg_len)
        .call(env.get(RuntimeFunction::Malloc))
        .local_set(ptr_local);
    for (offset, &byte) in msg_bytes.iter().enumerate() {
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
    body.local_get(ptr_local);
    body.i32_const(msg_len);
    body.call(env.get(RuntimeFunction::CreateError));
    // Free the temporary message buffer
    body.local_get(ptr_local);
    body.i32_const(msg_len);
    body.call(env.get(RuntimeFunction::Free));
    Ok(())
}

/// Compile a call to a registered extension function.
pub fn compile_extension_call(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if ctx.extensions.is_empty() {
        return emit_unknown_function_error(&call_expr.func_name, body, env, module);
    }

    // Determine call shape.
    let shape = match &call_expr.target {
        None => CallShape::Global,
        Some(target) => {
            if let Expr::Ident(name) = &target.expr {
                if ctx.extensions.namespaces.contains(name.as_str()) {
                    CallShape::Namespaced(name.as_str())
                } else {
                    CallShape::Receiver(Some(target.as_ref()))
                }
            } else {
                CallShape::Receiver(Some(target.as_ref()))
            }
        }
    };

    let (namespace_str, receiver_expr) = match &shape {
        CallShape::Global => (None, None),
        CallShape::Namespaced(ns) => (Some(*ns), None),
        CallShape::Receiver(expr) => (None, *expr),
    };

    // Look up in the registry.
    let key = ExtensionKey::new(
        namespace_str.map(|s: &str| s.to_string()),
        call_expr.func_name.clone(),
    );
    let decl = match ctx.extensions.by_name.get(&key) {
        Some(d) => d,
        None => {
            let full_name = match namespace_str {
                Some(ns) => format!("{}.{}", ns, call_expr.func_name),
                None => call_expr.func_name.clone(),
            };
            return emit_unknown_function_error(&full_name, body, env, module);
        }
    };

    // Validate call style.
    match &shape {
        CallShape::Receiver(_) => {
            if !decl.receiver_style {
                anyhow::bail!(
                    "Extension '{}' does not support receiver-style calls",
                    decl.function
                );
            }
        }
        CallShape::Global | CallShape::Namespaced(_) => {
            if !decl.global_style {
                anyhow::bail!(
                    "Extension '{}' does not support global-style calls",
                    decl.function
                );
            }
        }
    }

    // Count total args (receiver counts as arg 0).
    let total_args = call_expr.args.len()
        + if receiver_expr.is_some() {
            1usize
        } else {
            0usize
        };
    if total_args != decl.num_args {
        let full_name = match namespace_str {
            Some(ns) => format!("{}.{}", ns, call_expr.func_name),
            None => call_expr.func_name.clone(),
        };
        anyhow::bail!(
            "{} expects {} argument(s), got {}",
            full_name,
            decl.num_args,
            total_args
        );
    }

    if total_args > 4 {
        anyhow::bail!("Extension functions with more than 4 arguments are not supported");
    }

    // Get memory reference once.
    let memory_id = get_memory_id(module)?;

    // Emit (ns_ptr, ns_len).
    match namespace_str {
        Some(ns) => emit_string_const(ns, body, env, memory_id, module),
        None => {
            body.i32_const(0);
            body.i32_const(0);
        }
    }

    // Emit (method_ptr, method_len).
    emit_string_const(&call_expr.func_name, body, env, memory_id, module);

    // Emit receiver (if any) then remaining args.
    if let Some(recv) = receiver_expr {
        compile_expr(&recv.expr, body, env, ctx, module)?;
    }
    for arg in &call_expr.args {
        compile_expr(&arg.expr, body, env, ctx, module)?;
    }

    // Select the right fixed-arity wrapper and call it.
    let ext_fn = match total_args {
        0 => RuntimeFunction::ExtCall0,
        1 => RuntimeFunction::ExtCall1,
        2 => RuntimeFunction::ExtCall2,
        3 => RuntimeFunction::ExtCall3,
        4 => RuntimeFunction::ExtCall4,
        _ => unreachable!(),
    };
    body.call(env.get(ext_fn));

    Ok(())
}
