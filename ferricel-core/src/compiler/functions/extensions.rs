use cel::common::ast::{CallExpr, Expr};
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CallShape, CompilerContext, CompilerEnv, ExtensionKey},
    expr::compile_expr,
    helpers::{emit_string_const, get_memory_id},
};

/// Compile a call to a registered extension function.
pub fn compile_extension_call(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if ctx.extensions.is_empty() {
        anyhow::bail!("Unsupported function call: {}", call_expr.func_name);
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
    let decl = ctx.extensions.by_name.get(&key).ok_or_else(|| {
        let full_name = match namespace_str {
            Some(ns) => format!("{}.{}", ns, call_expr.func_name),
            None => call_expr.func_name.clone(),
        };
        anyhow::anyhow!("Unknown function: {}", full_name)
    })?;

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
