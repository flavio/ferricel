//! VAP (ValidatingAdmissionPolicy) compilation.
//!
//! Each CEL expression in the VAP spec (matchConditions, variables, validations,
//! messageExpressions) is compiled as an isolated sub-function. An orchestrating
//! `evaluate` function ties them together in the same order as the Kubernetes
//! VAP validator:
//!
//! 1. **params** (only when `spec.paramKind` is set). The runtime resolves the
//!    list of param objects from `paramRef`. See "Params" below.
//! 2. For each param (or once, when there is no `paramKind`):
//!    1. **matchConditions**. If any evaluates to `false`, this param is
//!       skipped. It is not a rejection.
//!    2. **variables**. Evaluated in declaration order. Each result is
//!       inserted into a `variables` map so later expressions can access
//!       `variables.<name>`. The map is rebuilt for each param.
//!    3. **validations**. Evaluated in order. The first `false` result
//!       returns a rejection response with the message and HTTP status code
//!       of that validation.
//! 3. When no param produced a rejection, return `{"accepted":true}`.
//!
//! ## Runtime errors
//!
//! If a `matchCondition` or `validation` expression evaluates to a CEL runtime
//! error, the module traps via `cel_abort` (like a plain CEL module does), so
//! the host receives an error from `evaluate`. An error is never treated as a
//! pass or as a rejection.
//!
//! On the host, [`Engine::eval`](crate::runtime::Engine::eval) returns an
//! error that downcasts to [`CelRuntimeError`](crate::CelRuntimeError).
//! When the `params` lookup fails, the error's `origin` is `kw.k8s.get` or
//! `kw.k8s.list`.
//!
//! A `variables` entry that evaluates to an error is stored as-is. The error
//! propagates only into the expressions that reference it (matching the lazy
//! semantics of Kubernetes). A `messageExpression` that errors or does not
//! produce a string falls back to the static `message`.
//!
//! ## Params
//!
//! The host supplies `paramRef` (the `spec.paramRef` of the binding) and
//! `request` in the bindings. The guest runtime reads them and calls the
//! host:
//!
//! - `paramRef.name`: one `kw.k8s.get` call. One param object.
//! - `paramRef.selector`: one `kw.k8s.list` call with the selector formatted
//!   as a label selector string. One param object per item.
//!
//! The namespace is `paramRef.namespace`. If that is empty, it is
//! `request.namespace`. If that is also empty, it is `""`.
//!
//! When the host call fails, or the list is empty, `parameterNotFoundAction`
//! decides the result. `Allow` accepts the request. Any other value (the
//! default is `Deny`) traps, and the host applies its `failurePolicy`.
//!
//! When several params match a selector and more than one produces a
//! rejection, the response holds the first rejection only. Kubernetes
//! aggregates every rejection message.
//!
//! The host must register a `kw.k8s` builder-chain implementation on the
//! `Engine`, for both `get` and `list`. The chain is declared via
//! [`kw_k8s_chain`] and injected automatically by
//! [`crate::compiler::Compiler::compile_vap_from_policy`].

use anyhow::Context as _;
use cel::parser::Parser;
use ferricel_types::{
    extensions::{BuilderChainDecl, BuilderStep, ExtensionDecl},
    functions::RuntimeFunction,
};
use k8s_openapi::api::admissionregistration::v1::{
    MatchCondition, ValidatingAdmissionPolicySpec, Validation, Variable,
};
use walrus::{FunctionBuilder, FunctionId, InstrSeqBuilder, LocalId, ValType, ir::InstrSeqId};

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::{compile_string_to_local, emit_set_variable, emit_string_const, get_memory_id},
};

// ─── kw.k8s builder chain ─────────────────────────────────────────────────────

/// Return the [`BuilderChainDecl`] for the `kw.k8s` fluent API.
///
/// This mirrors the Go `library.Kubernetes()` cel-go library:
///
/// ```text
/// kw.k8s.apiVersion(<string>) → kw.k8s.ClientBuilder
///   .kind(<string>)            → kw.k8s.Client
///   .namespace(<string>)       → kw.k8s.Client
///   .labelSelector(<string>)   → kw.k8s.Client
///   .fieldSelector(<string>)   → kw.k8s.Client
///   .fieldMask(<string>)       → kw.k8s.Client   (accumulates into array)
///   .list()                    → dyn  (host call)
///   .get(<string>)             → dyn  (host call)
/// ```
pub fn kw_k8s_chain() -> BuilderChainDecl {
    BuilderChainDecl {
        steps: vec![
            BuilderStep::Entry {
                function: "kw.k8s.apiVersion".to_string(),
                state_keys: vec!["apiVersion".to_string()],
                output_type: "kw.k8s.ClientBuilder".to_string(),
            },
            BuilderStep::Chain {
                function: "kind".to_string(),
                input_type: "kw.k8s.ClientBuilder".to_string(),
                state_keys: vec!["kind".to_string()],
                output_type: "kw.k8s.Client".to_string(),
                accumulate: false,
            },
            BuilderStep::Chain {
                function: "namespace".to_string(),
                input_type: "kw.k8s.Client".to_string(),
                state_keys: vec!["namespace".to_string()],
                output_type: "kw.k8s.Client".to_string(),
                accumulate: false,
            },
            BuilderStep::Chain {
                function: "labelSelector".to_string(),
                input_type: "kw.k8s.Client".to_string(),
                state_keys: vec!["labelSelector".to_string()],
                output_type: "kw.k8s.Client".to_string(),
                accumulate: false,
            },
            BuilderStep::Chain {
                function: "fieldSelector".to_string(),
                input_type: "kw.k8s.Client".to_string(),
                state_keys: vec!["fieldSelector".to_string()],
                output_type: "kw.k8s.Client".to_string(),
                accumulate: false,
            },
            BuilderStep::Chain {
                function: "fieldMask".to_string(),
                input_type: "kw.k8s.Client".to_string(),
                state_keys: vec!["fieldMasks".to_string()],
                output_type: "kw.k8s.Client".to_string(),
                accumulate: true,
            },
            BuilderStep::Terminal {
                function: "list".to_string(),
                input_type: "kw.k8s.Client".to_string(),
                extra_arg_keys: vec![],
                host_namespace: "kw.k8s".to_string(),
                host_function: "list".to_string(),
            },
            BuilderStep::Terminal {
                function: "get".to_string(),
                input_type: "kw.k8s.Client".to_string(),
                extra_arg_keys: vec!["name".to_string()],
                host_namespace: "kw.k8s".to_string(),
                host_function: "get".to_string(),
            },
        ],
    }
}

/// Return the [`ExtensionDecl`] for the `kw.k8s.get` terminal step.
///
/// Pass this to [`runtime::Builder::with_extension`] to register a host
/// implementation that resolves single-resource fetches made by policies
/// compiled with [`kw_k8s_chain`].
///
/// [`runtime::Builder::with_extension`]: crate::runtime::Builder::with_extension
#[cfg_attr(docsrs, doc(cfg(feature = "k8s-vap")))]
pub fn kw_k8s_get_extension() -> ExtensionDecl {
    ExtensionDecl {
        namespace: Some("kw.k8s".to_string()),
        function: "get".to_string(),
        global_style: false,
        receiver_style: false,
        num_args: 1,
    }
}

/// Return the [`ExtensionDecl`] for the `kw.k8s.list` terminal step.
///
/// Pass this to [`runtime::Builder::with_extension`] to register a host
/// implementation that resolves list fetches made by policies compiled
/// with [`kw_k8s_chain`].
///
/// [`runtime::Builder::with_extension`]: crate::runtime::Builder::with_extension
#[cfg_attr(docsrs, doc(cfg(feature = "k8s-vap")))]
pub fn kw_k8s_list_extension() -> ExtensionDecl {
    ExtensionDecl {
        namespace: Some("kw.k8s".to_string()),
        function: "list".to_string(),
        global_style: false,
        receiver_style: false,
        num_args: 1,
    }
}

// ─── HTTP reason codes ────────────────────────────────────────────────────────

/// Map a VAP `reason` string to an HTTP status code.
pub fn reason_to_http_code(reason: Option<&str>) -> i32 {
    match reason {
        Some("Unauthorized") => 401,
        Some("Forbidden") => 403,
        Some("RequestEntityTooLarge") => 413,
        // "Invalid" and anything unknown → 422 Unprocessable Entity
        _ => 422,
    }
}

// ─── Compiled validation pair ─────────────────────────────────────────────────

/// A compiled validation expression together with its optional `messageExpression`
/// sub-function.
struct CompiledValidation {
    /// Wasm function for the validation `expression` (returns `*mut CelValue`).
    id: FunctionId,
    /// Wasm function for the `messageExpression`, if one was specified.
    msg_expr_fn: Option<FunctionId>,
}

// ─── Orchestrator arguments ───────────────────────────────────────────────────

/// All inputs to [`build_orchestrator`], collected into a named struct to avoid
/// a long positional argument list.
struct OrchestratorArgs<'a> {
    /// The VAP spec being compiled (used to read static field values such as
    /// `reason` and `message` from each [`Validation`]).
    spec: &'a ValidatingAdmissionPolicySpec,
    /// Compiled Wasm functions for the `matchConditions`, in declaration order.
    match_conditions_fns: Vec<FunctionId>,
    /// Compiled Wasm functions for the `variables`, in declaration order.
    variables_fns: Vec<FunctionId>,
    /// Compiled validation expression + optional messageExpression pairs, in
    /// declaration order.
    validations: Vec<CompiledValidation>,
}

// ─── Core compilation ─────────────────────────────────────────────────────────

/// Compile a `ValidatingAdmissionPolicySpec` into an orchestrating Wasm function
/// and return its `FunctionId`.
pub(crate) fn build_vap_evaluate_function(
    module: &mut walrus::Module,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    spec: &ValidatingAdmissionPolicySpec,
) -> Result<FunctionId, anyhow::Error> {
    // Pre-compile all sub-functions (each CEL expression → a Wasm function)
    let match_conditions_fns = compile_match_condition_fns(
        module,
        env,
        ctx,
        spec.match_conditions.as_deref().unwrap_or(&[]),
    )?;
    let variables_fns =
        compile_variable_fns(module, env, ctx, spec.variables.as_deref().unwrap_or(&[]))?;
    let validations =
        compile_validation_fns(module, env, ctx, spec.validations.as_deref().unwrap_or(&[]))?;

    build_orchestrator(
        module,
        env,
        OrchestratorArgs {
            spec,
            match_conditions_fns,
            variables_fns,
            validations,
        },
    )
}

// ─── Sub-function compilation ─────────────────────────────────────────────────

fn compile_match_condition_fns(
    module: &mut walrus::Module,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    conditions: &[MatchCondition],
) -> Result<Vec<FunctionId>, anyhow::Error> {
    conditions
        .iter()
        .map(|mc| {
            compile_sub_fn(&mc.expression, module, env, ctx)
                .with_context(|| format!("matchCondition '{}': compile error", mc.name))
        })
        .collect()
}

fn compile_variable_fns(
    module: &mut walrus::Module,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    variables: &[Variable],
) -> Result<Vec<FunctionId>, anyhow::Error> {
    variables
        .iter()
        .map(|v| {
            compile_sub_fn(&v.expression, module, env, ctx)
                .with_context(|| format!("variable '{}': compile error", v.name))
        })
        .collect()
}

/// Compile each validation's `expression` and optional `messageExpression` into
/// a [`CompiledValidation`].
fn compile_validation_fns(
    module: &mut walrus::Module,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    validations: &[Validation],
) -> Result<Vec<CompiledValidation>, anyhow::Error> {
    validations
        .iter()
        .enumerate()
        .map(|(i, val)| {
            let id = compile_sub_fn(&val.expression, module, env, ctx)
                .with_context(|| format!("validation[{i}]: compile error"))?;

            let msg_expr_fn = val
                .message_expression
                .as_deref()
                .map(|expr| {
                    compile_sub_fn(expr, module, env, ctx).with_context(|| {
                        format!("validation[{i}] messageExpression: compile error")
                    })
                })
                .transpose()?;

            Ok(CompiledValidation { id, msg_expr_fn })
        })
        .collect()
}

/// Compile a single CEL expression into a Wasm sub-function `() -> i32`
/// (returns a `*mut CelValue`).
fn compile_sub_fn(
    cel_code: &str,
    module: &mut walrus::Module,
    env: &CompilerEnv,
    ctx: &CompilerContext,
) -> Result<FunctionId, anyhow::Error> {
    let ast = Parser::new()
        .enable_optional_syntax(true)
        .parse(cel_code)
        .map_err(|e| anyhow::anyhow!("CEL parse error: {:?}", e))
        .with_context(|| format!("failed to compile CEL expression: {cel_code:?}"))?;

    let mut func = FunctionBuilder::new(&mut module.types, &[], &[ValType::I32]);
    let mut body = func.func_body();
    compile_expr(&ast.expr, &mut body, env, ctx, module)
        .with_context(|| format!("failed to compile CEL expression: {cel_code:?}"))?;
    Ok(func.finish(vec![], &mut module.funcs))
}

// ─── Orchestrating function ───────────────────────────────────────────────────

/// Wasm locals that the orchestrator and [`emit_param_evaluation`] share.
struct OrchestratorLocals {
    /// Scratch `*mut CelValue` for the result of the last sub-function call.
    val: LocalId,
    /// The `variables` map for the current param.
    variables_map: LocalId,
}

/// Build the exported `evaluate(bindings: i64) -> i64` function.
///
/// Emitted pseudo-code:
///
/// ```text
/// bindings = deserialize(arg); init_bindings(bindings)
/// if spec.paramKind is set:
///     list = cel_vap_resolve_params(apiVersion, kind); abort_if_error(list)
///     n = cel_array_len(list); i = 0
///     block exit {
///       loop next {
///         br_if exit (i >= n)
///         set_variable("params", cel_array_get(list, i))
///         i += 1
///         block skip { <matchConditions, variables, validations> }
///         br next
///       }
///     }
/// else:
///     block skip { <matchConditions, variables, validations> }
/// return serialize_accept()
/// ```
///
/// A false `matchCondition` branches to `skip`. A false validation returns a
/// rejection response from inside the block. When the loop ends, or the
/// param list is empty, the function returns an acceptance response.
fn build_orchestrator(
    module: &mut walrus::Module,
    env: &CompilerEnv,
    args: OrchestratorArgs<'_>,
) -> Result<FunctionId, anyhow::Error> {
    let mut func = FunctionBuilder::new(&mut module.types, &[ValType::I64], &[ValType::I64]);
    let bindings_arg = module.locals.add(ValType::I64);
    let locals = OrchestratorLocals {
        val: module.locals.add(ValType::I32),
        variables_map: module.locals.add(ValType::I32),
    };
    let mut body = func.func_body();

    // 1. Deserialize + init bindings
    body.local_get(bindings_arg)
        .call(env.get(RuntimeFunction::DeserializeJson))
        .call(env.get(RuntimeFunction::InitBindings));

    match &args.spec.param_kind {
        Some(param_kind) => {
            let api_version = param_kind.api_version.as_deref().unwrap_or("");
            let kind = param_kind.kind.as_deref().unwrap_or("");
            emit_params_loop(&mut body, api_version, kind, &args, env, module, &locals)?;
        }
        None => {
            let skip_id = body.dangling_instr_seq(None).id();
            body.instr(walrus::ir::Block { seq: skip_id });
            let mut skip_body = body.instr_seq(skip_id);
            emit_param_evaluation(&mut skip_body, skip_id, &args, env, module, &locals)?;
        }
    }

    // Every param passed (or was skipped) → accept.
    body.call(env.get(RuntimeFunction::VapSerializeAccept));

    Ok(func.finish(vec![bindings_arg], &mut module.funcs))
}

/// Emit the `params` resolution and the per-param loop. See
/// [`build_orchestrator`] for the pseudo-code.
fn emit_params_loop(
    body: &mut InstrSeqBuilder,
    api_version: &str,
    kind: &str,
    args: &OrchestratorArgs<'_>,
    env: &CompilerEnv,
    module: &mut walrus::Module,
    locals: &OrchestratorLocals,
) -> Result<(), anyhow::Error> {
    let mem = get_memory_id(module)?;
    let list_local = module.locals.add(ValType::I32);
    let len_local = module.locals.add(ValType::I32);
    let index_local = module.locals.add(ValType::I32);
    let param_local = module.locals.add(ValType::I32);

    // list = cel_vap_resolve_params(apiVersion, kind); abort_if_error(list)
    emit_string_const(api_version, body, env, mem, module);
    emit_string_const(kind, body, env, mem, module);
    body.call(env.get(RuntimeFunction::VapResolveParams))
        .local_set(list_local)
        .local_get(list_local)
        .call(env.get(RuntimeFunction::AbortIfError));

    // n = cel_array_len(list); i = 0
    body.local_get(list_local)
        .call(env.get(RuntimeFunction::ArrayLen))
        .local_set(len_local);
    body.i32_const(0).local_set(index_local);

    // block exit { loop next { ... } }
    let exit_id = body.dangling_instr_seq(None).id();
    let loop_id = body.dangling_instr_seq(None).id();
    let skip_id = body.dangling_instr_seq(None).id();

    body.instr(walrus::ir::Block { seq: exit_id });
    body.instr_seq(exit_id)
        .instr(walrus::ir::Loop { seq: loop_id });

    {
        let mut loop_body = body.instr_seq(loop_id);

        // br_if exit (i >= n)
        loop_body
            .local_get(index_local)
            .local_get(len_local)
            .binop(walrus::ir::BinaryOp::I32GeU)
            .instr(walrus::ir::BrIf { block: exit_id });

        // set_variable("params", cel_array_get(list, i))
        loop_body
            .local_get(list_local)
            .local_get(index_local)
            .call(env.get(RuntimeFunction::ArrayGet))
            .local_set(param_local);
        emit_set_variable("params", param_local, &mut loop_body, env, module)?;

        // i += 1 (before the body, so `skip` only needs to fall through)
        loop_body
            .local_get(index_local)
            .i32_const(1)
            .binop(walrus::ir::BinaryOp::I32Add)
            .local_set(index_local);

        // block skip { ... }; br next
        loop_body.instr(walrus::ir::Block { seq: skip_id });
        loop_body.instr(walrus::ir::Br { block: loop_id });
    }

    let mut skip_body = body.instr_seq(skip_id);
    emit_param_evaluation(&mut skip_body, skip_id, args, env, module, locals)
}

/// Emit the evaluation of one param: `matchConditions`, `variables`, then
/// `validations`.
///
/// A false `matchCondition` branches to `skip_id`, the enclosing block. A
/// false validation returns a rejection response. When every validation
/// passes, control falls through to the end of `body`.
fn emit_param_evaluation(
    body: &mut InstrSeqBuilder,
    skip_id: InstrSeqId,
    args: &OrchestratorArgs<'_>,
    env: &CompilerEnv,
    module: &mut walrus::Module,
    locals: &OrchestratorLocals,
) -> Result<(), anyhow::Error> {
    // 1. matchConditions — false → skip this param.
    //    A CEL runtime error traps so the host can apply `failurePolicy`.
    for &fn_id in &args.match_conditions_fns {
        body.call(fn_id)
            .local_set(locals.val)
            .local_get(locals.val)
            .call(env.get(RuntimeFunction::AbortIfError))
            .local_get(locals.val)
            .call(env.get(RuntimeFunction::IsStrictlyFalse))
            .instr(walrus::ir::BrIf { block: skip_id });
    }

    // 2. Create a fresh `variables` map.
    body.call(env.get(RuntimeFunction::CreateMap))
        .local_set(locals.variables_map);

    // 3. Evaluate variables in order, insert each into the map, then update the
    //    "variables" binding so that subsequent variable expressions (and all
    //    validation expressions) can access `variables.<name>`.
    //
    //    We call emit_set_variable *after each insertion* so that later variable
    //    expressions can reference earlier ones via `variables.X` (per K8s spec).
    let variables = args.spec.variables.as_deref().unwrap_or(&[]);
    for (i, var) in variables.iter().enumerate() {
        body.call(args.variables_fns[i]).local_set(locals.val);

        let key_local = compile_string_to_local(&var.name, body, env, module)?;
        body.local_get(locals.variables_map);
        body.local_get(key_local);
        body.local_get(locals.val);
        body.call(env.get(RuntimeFunction::MapInsert));

        // Re-register the (now-updated) map so subsequent lookups see the new entry.
        emit_set_variable("variables", locals.variables_map, body, env, module)?;
    }

    // If there are no variables, still register an empty map so that
    // expressions that reference `variables` (even if unused) don't error.
    if variables.is_empty() {
        emit_set_variable("variables", locals.variables_map, body, env, module)?;
    }

    // 4. Validations — pre-allocate static message locals, then emit conditionals
    let validations_spec = args.spec.validations.as_deref().unwrap_or(&[]);
    for (i, compiled) in args.validations.iter().enumerate() {
        let val_spec = &validations_spec[i];
        let http_code = reason_to_http_code(val_spec.reason.as_deref());
        let msg_expr_fn = compiled.msg_expr_fn;

        // Evaluate the validation expression. A CEL runtime error must surface
        // to the host (trap), not be mistaken for a non-`false` (passing) result.
        body.call(compiled.id)
            .local_set(locals.val)
            .local_get(locals.val)
            .call(env.get(RuntimeFunction::AbortIfError));

        // Pre-compute the static message (needs &mut module, so must be outside
        // the closure). This is always emitted: it is the message when no
        // `messageExpression` is set, and the fallback when the
        // `messageExpression` errors or does not produce a string.
        let text = val_spec
            .message
            .clone()
            .unwrap_or_else(|| format!("failed expression: {}", val_spec.expression));
        let static_msg_local = compile_string_to_local(&text, body, env, module)?;

        body.local_get(locals.val)
            .call(env.get(RuntimeFunction::IsStrictlyFalse));
        body.if_else(
            None,
            move |then| {
                // message_ptr: messageExpression result, or null if none
                if let Some(fn_id) = msg_expr_fn {
                    then.call(fn_id);
                } else {
                    then.i32_const(0);
                }
                // fallback_ptr: static message
                then.local_get(static_msg_local)
                    .i32_const(http_code)
                    .call(env.get(RuntimeFunction::VapSerializeReject))
                    .return_();
            },
            |_| {},
        );
    }

    Ok(())
}
