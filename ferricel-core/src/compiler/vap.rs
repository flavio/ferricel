//! VAP (ValidatingAdmissionPolicy) compilation.
//!
//! Each CEL expression in the VAP spec (matchConditions, variables, validations,
//! messageExpressions) is compiled as an isolated sub-function.  An orchestrating
//! `evaluate` function ties them together following K8s VAP evaluation order:
//!
//! 1. **matchConditions** — if any evaluates to `false`, the policy does not
//!    apply; return `{"accepted":true}` immediately (policy skipped, not a
//!    rejection).
//! 2. **variables** — evaluated in declaration order; each result is inserted
//!    into a `variables` map so subsequent expressions can access
//!    `variables.<name>`.
//! 3. **validations** — evaluated in order; first `false` result returns a
//!    rejection response with the appropriate message and HTTP status code.
//!
//! ## K8s resource fetching (params)
//!
//! The host must register a `kw.k8s` builder-chain implementation on the
//! `Engine`. The chain is declared via [`kw_k8s_chain`] and injected
//! automatically by [`compile_vap_from_spec`].

use anyhow::Context as _;
use cel::parser::Parser;
use ferricel_types::{
    extensions::{BuilderChainDecl, BuilderStep, ExtensionDecl},
    functions::RuntimeFunction,
};
use k8s_openapi::api::admissionregistration::v1::{
    MatchCondition, ParamKind, ValidatingAdmissionPolicySpec, Validation, Variable,
};
use walrus::{FunctionBuilder, FunctionId, LocalId, ValType};

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
    helpers::{
        compile_string_to_local, emit_get_variable, emit_set_variable, emit_string_const,
        get_memory_id,
    },
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
                state_key: "apiVersion".to_string(),
                output_type: "kw.k8s.ClientBuilder".to_string(),
            },
            BuilderStep::Chain {
                function: "kind".to_string(),
                input_type: "kw.k8s.ClientBuilder".to_string(),
                state_key: "kind".to_string(),
                output_type: "kw.k8s.Client".to_string(),
                accumulate: false,
            },
            BuilderStep::Chain {
                function: "namespace".to_string(),
                input_type: "kw.k8s.Client".to_string(),
                state_key: "namespace".to_string(),
                output_type: "kw.k8s.Client".to_string(),
                accumulate: false,
            },
            BuilderStep::Chain {
                function: "labelSelector".to_string(),
                input_type: "kw.k8s.Client".to_string(),
                state_key: "labelSelector".to_string(),
                output_type: "kw.k8s.Client".to_string(),
                accumulate: false,
            },
            BuilderStep::Chain {
                function: "fieldSelector".to_string(),
                input_type: "kw.k8s.Client".to_string(),
                state_key: "fieldSelector".to_string(),
                output_type: "kw.k8s.Client".to_string(),
                accumulate: false,
            },
            BuilderStep::Chain {
                function: "fieldMask".to_string(),
                input_type: "kw.k8s.Client".to_string(),
                state_key: "fieldMasks".to_string(),
                output_type: "kw.k8s.Client".to_string(),
                accumulate: true,
            },
            BuilderStep::Terminal {
                function: "list".to_string(),
                input_type: "kw.k8s.Client".to_string(),
                extra_arg_key: None,
                host_namespace: "kw.k8s".to_string(),
                host_function: "list".to_string(),
            },
            BuilderStep::Terminal {
                function: "get".to_string(),
                input_type: "kw.k8s.Client".to_string(),
                extra_arg_key: Some("name".to_string()),
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

fn build_orchestrator(
    module: &mut walrus::Module,
    env: &CompilerEnv,
    args: OrchestratorArgs<'_>,
) -> Result<FunctionId, anyhow::Error> {
    let mut func = FunctionBuilder::new(&mut module.types, &[ValType::I64], &[ValType::I64]);
    let bindings_arg = module.locals.add(ValType::I64);
    let val_local = module.locals.add(ValType::I32);
    let variables_map = module.locals.add(ValType::I32);
    let mut body = func.func_body();

    // 1. Deserialize + init bindings
    body.local_get(bindings_arg)
        .call(env.get(RuntimeFunction::DeserializeJson))
        .call(env.get(RuntimeFunction::InitBindings));

    // 2. matchConditions — false → policy skipped → accept
    for &fn_id in &args.match_conditions_fns {
        body.call(fn_id)
            .local_set(val_local)
            .local_get(val_local)
            .call(env.get(RuntimeFunction::IsStrictlyFalse));
        body.if_else(
            None,
            |then| {
                then.call(env.get(RuntimeFunction::VapSerializeAccept))
                    .return_();
            },
            |_| {},
        );
    }

    // 3. Create `variables` map.
    body.call(env.get(RuntimeFunction::CreateMap))
        .local_set(variables_map);

    // 4. Evaluate variables in order, insert each into the map, then update the
    //    "variables" binding so that subsequent variable expressions (and all
    //    validation expressions) can access `variables.<name>`.
    //
    //    We call emit_set_variable *after each insertion* so that later variable
    //    expressions can reference earlier ones via `variables.X` (per K8s spec).
    let variables = args.spec.variables.as_deref().unwrap_or(&[]);
    for (i, var) in variables.iter().enumerate() {
        body.call(args.variables_fns[i]).local_set(val_local);

        let key_local = compile_string_to_local(&var.name, &mut body, env, module)?;
        body.local_get(variables_map);
        body.local_get(key_local);
        body.local_get(val_local);
        body.call(env.get(RuntimeFunction::MapInsert));

        // Re-register the (now-updated) map so subsequent lookups see the new entry.
        emit_set_variable("variables", variables_map, &mut body, env, module)?;
    }

    // If there are no variables, still register an empty map so that
    // expressions that reference `variables` (even if unused) don't error.
    if variables.is_empty() {
        emit_set_variable("variables", variables_map, &mut body, env, module)?;
    }

    // 5. params (after variables so expressions can reference variables)
    if let Some(ref pk) = args.spec.param_kind {
        emit_fetch_params(pk, &mut body, env, module)?;
    }

    // 6. Validations — pre-allocate static message locals, then emit conditionals
    let validations_spec = args.spec.validations.as_deref().unwrap_or(&[]);
    for (i, compiled) in args.validations.iter().enumerate() {
        let val_spec = &validations_spec[i];
        let http_code = reason_to_http_code(val_spec.reason.as_deref());
        let msg_expr_fn = compiled.msg_expr_fn;

        // Evaluate the validation expression
        body.call(compiled.id).local_set(val_local);

        // Pre-compute static message (needs &mut module, so must be outside closure)
        let static_msg_local: Option<LocalId> = if msg_expr_fn.is_none() {
            let text = val_spec
                .message
                .clone()
                .unwrap_or_else(|| format!("failed expression: {}", val_spec.expression));
            Some(compile_string_to_local(&text, &mut body, env, module)?)
        } else {
            None
        };

        body.local_get(val_local)
            .call(env.get(RuntimeFunction::IsStrictlyFalse));
        body.if_else(
            None,
            move |then| {
                if let Some(fn_id) = msg_expr_fn {
                    then.call(fn_id);
                } else if let Some(loc) = static_msg_local {
                    then.local_get(loc);
                }
                then.i32_const(http_code)
                    .call(env.get(RuntimeFunction::VapSerializeReject))
                    .return_();
            },
            |_| {},
        );
    }

    // 7. All validations passed → accept
    body.call(env.get(RuntimeFunction::VapSerializeAccept));

    Ok(func.finish(vec![bindings_arg], &mut module.funcs))
}

// ─── K8s resource fetch emitters ─────────────────────────────────────────────

/// Emit code to fetch `params` via `kw.k8s.apiVersion(...).kind(...).get(name)`.
///
/// The `paramRef.name` and `paramRef.namespace` are read from the bindings map
/// at runtime (injected by the host).
///
/// Emitted pseudo-code:
/// ```text
/// paramRef = get_variable("paramRef")
/// name_val = paramRef["name"]
/// ns_val   = paramRef["namespace"]
///
/// // kw.k8s.apiVersion(api_version).kind(kind).namespace(ns).get(name)
/// builder = cel_builder_step(null, "kw.k8s.ClientBuilder", "apiVersion", api_version_str, 0)
/// builder = cel_builder_step(builder, "kw.k8s.Client", "kind", kind_str, 0)
/// builder = cel_builder_step(builder, "kw.k8s.Client", "namespace", ns_val, 0)
/// builder = cel_builder_step(builder, "kw.k8s.Client", "name", name_val, 0)
/// params  = ExtCall1("kw.k8s", "get", builder)
/// set_variable("params", params)
/// ```
fn emit_fetch_params(
    param_kind: &ParamKind,
    body: &mut walrus::InstrSeqBuilder,
    env: &CompilerEnv,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    let mem = get_memory_id(module)?;

    let param_ref = module.locals.add(ValType::I32);
    let name_val = module.locals.add(ValType::I32);
    let ns_val = module.locals.add(ValType::I32);
    let builder = module.locals.add(ValType::I32);
    let result = module.locals.add(ValType::I32);

    // Read paramRef from bindings
    emit_get_variable("paramRef", body, env, module)?;
    body.local_set(param_ref);

    // Extract paramRef.name and paramRef.namespace
    body.local_get(param_ref);
    emit_string_const("name", body, env, mem, module);
    body.call(env.get(RuntimeFunction::GetField))
        .local_set(name_val);

    body.local_get(param_ref);
    emit_string_const("namespace", body, env, mem, module);
    body.call(env.get(RuntimeFunction::GetField))
        .local_set(ns_val);

    let api_version = param_kind.api_version.as_deref().unwrap_or("");
    let kind = param_kind.kind.as_deref().unwrap_or("");

    // Step 1: kw.k8s.apiVersion(api_version) → ClientBuilder
    {
        let api_version_local = compile_string_to_local(api_version, body, env, module)?;
        body.i32_const(0); // null receiver
        emit_string_const("kw.k8s.ClientBuilder", body, env, mem, module);
        emit_string_const("apiVersion", body, env, mem, module);
        body.local_get(api_version_local);
        body.i32_const(0); // accumulate = false
        body.call(env.get(RuntimeFunction::BuilderStepCall))
            .local_set(builder);
    }

    // Step 2: .kind(kind) → Client
    {
        let kind_local = compile_string_to_local(kind, body, env, module)?;
        body.local_get(builder);
        emit_string_const("kw.k8s.Client", body, env, mem, module);
        emit_string_const("kind", body, env, mem, module);
        body.local_get(kind_local);
        body.i32_const(0);
        body.call(env.get(RuntimeFunction::BuilderStepCall))
            .local_set(builder);
    }

    // Step 3: .namespace(ns_val) — ns_val is a *mut CelValue from runtime
    {
        body.local_get(builder);
        emit_string_const("kw.k8s.Client", body, env, mem, module);
        emit_string_const("namespace", body, env, mem, module);
        body.local_get(ns_val);
        body.i32_const(0);
        body.call(env.get(RuntimeFunction::BuilderStepCall))
            .local_set(builder);
    }

    // Step 4: fold name into the map so the host gets it
    {
        body.local_get(builder);
        emit_string_const("kw.k8s.Client", body, env, mem, module);
        emit_string_const("name", body, env, mem, module);
        body.local_get(name_val);
        body.i32_const(0);
        body.call(env.get(RuntimeFunction::BuilderStepCall))
            .local_set(builder);
    }

    // Terminal: ExtCall1("kw.k8s", "get", builder)
    emit_string_const("kw.k8s", body, env, mem, module);
    emit_string_const("get", body, env, mem, module);
    body.local_get(builder);
    body.call(env.get(RuntimeFunction::ExtCall1))
        .local_set(result);

    emit_set_variable("params", result, body, env, module)
}
