//! VAP `failurePolicy` support.
//!
//! Kubernetes applies `failurePolicy` to each `matchConditions` and
//! `validations` expression on its own. Under `Ignore`, when an expression
//! evaluates to a CEL runtime error, the module skips that expression and
//! continues with the next expression, or the next param. Under `Fail`, the
//! first error stops the evaluation.
//!
//! The host passes the policy in the `failurePolicy` binding. The module
//! never reads `spec.failurePolicy` of the VAP.
//!
//! - [`cel_vap_reset`] clears the per-evaluation state and reads the
//!   binding. The orchestrator calls it once, right after
//!   `cel_init_bindings`.
//! - [`cel_vap_expression_errored`] decides what to do with the result of
//!   one expression. Under `Ignore`, it records a warning. The response
//!   serializer adds the warning to the `warnings` field.

use std::cell::UnsafeCell;

use super::{get_str, read_binding};
use crate::{
    error::{CelError, CelResult, abort_with_cel_error},
    globals::GlobalCell,
    memory::read_str,
    types::{CelMapKey, CelValue},
};

/// The `failurePolicy` of a VAP, as the host passes it in the bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailurePolicy {
    /// When an expression evaluates to a CEL runtime error, the module traps.
    Fail,
    /// When an expression evaluates to a CEL runtime error, the module skips
    /// the expression and records a warning.
    Ignore,
}

/// The policy of the current evaluation. Set by [`cel_vap_reset`].
static FAILURE_POLICY: GlobalCell<FailurePolicy> = GlobalCell(UnsafeCell::new(FailurePolicy::Fail));

/// The warnings of the current evaluation. Cleared by [`cel_vap_reset`].
static WARNINGS: GlobalCell<Vec<String>> = GlobalCell(UnsafeCell::new(Vec::new()));

/// Parse the `failurePolicy` binding.
///
/// A missing or `null` binding means `Fail`. As a result, a host that does
/// not send the binding keeps the behavior of a module without
/// `failurePolicy` support. Any other non-string value is an error. A
/// string that is not `Fail` or `Ignore` is an error. The value is
/// case-sensitive, like the Kubernetes field.
fn parse_failure_policy(value: Option<&CelValue>) -> CelResult<FailurePolicy> {
    match value {
        None | Some(CelValue::Null) => Ok(FailurePolicy::Fail),
        Some(CelValue::String(s)) if s == "Fail" => Ok(FailurePolicy::Fail),
        Some(CelValue::String(s)) if s == "Ignore" => Ok(FailurePolicy::Ignore),
        Some(CelValue::String(s)) => Err(CelError::new(format!(
            "failurePolicy binding must be \"Fail\" or \"Ignore\", got {s:?}"
        ))),
        Some(_) => Err(CelError::new(
            "failurePolicy binding must be a string (\"Fail\" or \"Ignore\")",
        )),
    }
}

/// Reset the per-evaluation VAP state.
///
/// This function clears the warnings and reads `failurePolicy` from the
/// bindings. The state is global to the module instance. A host can run
/// several `evaluate` calls on one instance. As a result, the orchestrator
/// must call this function once per `evaluate`, right after
/// `cel_init_bindings`.
///
/// If the binding holds an invalid value, the module traps with a CEL
/// runtime error. This is the single, early place that reports an invalid
/// value.
///
/// # Safety
/// `cel_init_bindings` must have run before this function.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_vap_reset() {
    let policy = match parse_failure_policy(read_binding("failurePolicy").as_ref()) {
        Ok(policy) => policy,
        Err(err) => abort_with_cel_error(&err),
    };
    *FAILURE_POLICY.0.get() = policy;
    (*WARNINGS.0.get()).clear();
}

/// Borrow the warnings of the current evaluation.
///
/// # Safety
/// Must only be called from the single-threaded Wasm guest environment.
pub(crate) unsafe fn warnings() -> &'static [String] {
    unsafe { &*WARNINGS.0.get() }
}

/// Decide what to do with the result of one `matchConditions` or
/// `validations` expression.
///
/// # Parameters
/// - `value`: the result of the expression. Not consumed.
/// - `label_ptr`, `label_len`: UTF-8 bytes that name the expression, for
///   example `validation[0]` or `matchCondition 'is-deployment'`. The
///   compiler owns the text.
///
/// # Returns
/// - `0` when `value` is not a `CelValue::Error`. The caller continues with
///   the strictly-false test.
/// - `1` when `value` is an error and `failurePolicy` is `Ignore`. The
///   function recorded a warning. The caller must skip the expression: the
///   next validation runs, or the next param for a matchCondition.
/// - No return when `value` is an error and `failurePolicy` is `Fail`. The
///   module traps via `cel_abort`, like `cel_abort_if_error`.
///
/// # Safety
/// - `value` must be null or a valid `CelValue` pointer.
/// - When `label_len > 0`, `label_ptr` must point to `label_len` valid
///   UTF-8 bytes.
/// - `cel_vap_reset` must have run in this evaluation.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_vap_expression_errored(
    value: *mut CelValue,
    label_ptr: *const u8,
    label_len: i32,
) -> i32 {
    if value.is_null() {
        return 0;
    }
    let CelValue::Error(err) = &*value else {
        return 0;
    };

    let policy = *FAILURE_POLICY.0.get();
    if policy == FailurePolicy::Fail {
        abort_with_cel_error(err);
    }

    let label = read_str(label_ptr, label_len);
    let params = read_binding("params");
    let warnings = &mut *WARNINGS.0.get();
    warnings.push(skip_warning(err, label, params.as_ref()));
    1
}

/// Build the warning text for one skipped expression.
///
/// - A `validation[<i>]` label gives
///   `The module skipped validation[<i>] because the expression evaluated to an error (failurePolicy is Ignore): <message>`.
///   With `params`, `for <params>` follows the label. See [`params_label`].
/// - Any other label (a matchCondition) gives
///   `The module skipped <params> because <label> evaluated to an error (failurePolicy is Ignore): <message>`,
///   or `skipped the policy` when there is no `params` binding.
fn skip_warning(err: &CelError, label: &str, params: Option<&CelValue>) -> String {
    let message = &err.message;
    let reason = "(failurePolicy is Ignore)";
    if label.starts_with("validation[") {
        let scope = params
            .map(|p| format!(" for {}", params_label(p)))
            .unwrap_or_default();
        format!(
            "The module skipped {label}{scope} because the expression evaluated to an error {reason}: {message}"
        )
    } else {
        let what = params
            .map(params_label)
            .unwrap_or_else(|| "the policy".to_string());
        format!(
            "The module skipped {what} because {label} evaluated to an error {reason}: {message}"
        )
    }
}

/// The name of the current param object, for a warning.
///
/// `params <namespace>/<name>` when `metadata` holds both, `params <name>`
/// when it holds the name only, `the current params` otherwise.
fn params_label(params: &CelValue) -> String {
    let metadata = match params {
        CelValue::Object(map) => match map.get(&CelMapKey::from("metadata")) {
            Some(CelValue::Object(metadata)) => Some(metadata),
            _ => None,
        },
        _ => None,
    };
    let name = metadata.and_then(|m| get_str(m, "name"));
    let namespace = metadata
        .and_then(|m| get_str(m, "namespace"))
        .filter(|ns| !ns.is_empty());
    match (namespace, name) {
        (Some(ns), Some(name)) => format!("params {ns}/{name}"),
        (None, Some(name)) => format!("params {name}"),
        _ => "the current params".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;

    use super::*;
    use crate::test_helpers::make_json;

    #[rstest]
    #[case::missing(None, Ok(FailurePolicy::Fail))]
    #[case::null(Some(json!(null)), Ok(FailurePolicy::Fail))]
    #[case::fail(Some(json!("Fail")), Ok(FailurePolicy::Fail))]
    #[case::ignore(Some(json!("Ignore")), Ok(FailurePolicy::Ignore))]
    #[case::lowercase(
        Some(json!("ignore")),
        Err("failurePolicy binding must be \"Fail\" or \"Ignore\", got \"ignore\"")
    )]
    #[case::bool(
        Some(json!(true)),
        Err("failurePolicy binding must be a string (\"Fail\" or \"Ignore\")")
    )]
    #[case::int(
        Some(json!(1)),
        Err("failurePolicy binding must be a string (\"Fail\" or \"Ignore\")")
    )]
    fn failure_policy_is_parsed(
        #[case] value: Option<serde_json::Value>,
        #[case] expected: Result<FailurePolicy, &str>,
    ) {
        let value = value.map(make_json);
        let result = parse_failure_policy(value.as_ref());
        match expected {
            Ok(policy) => assert_eq!(result.unwrap(), policy),
            Err(message) => assert_eq!(result.unwrap_err().message, message),
        }
    }

    fn cm(metadata: serde_json::Value) -> CelValue {
        make_json(json!({ "kind": "ConfigMap", "metadata": metadata }))
    }

    #[rstest]
    #[case::namespace_and_name(cm(json!({ "namespace": "default", "name": "limits" })), "params default/limits")]
    #[case::name_only(cm(json!({ "name": "limits" })), "params limits")]
    #[case::empty_namespace(cm(json!({ "namespace": "", "name": "limits" })), "params limits")]
    #[case::no_metadata(make_json(json!({ "kind": "ConfigMap" })), "the current params")]
    #[case::not_a_map(make_json(json!("x")), "the current params")]
    fn params_label_is_built(#[case] params: CelValue, #[case] expected: &str) {
        assert_eq!(params_label(&params), expected);
    }

    /// The warning names the actor (the module), the skipped thing, the cause,
    /// and carries the error message unchanged.
    #[rstest]
    #[case::validation_without_params(
        CelError::new("no such key: 'count'"),
        "validation[0]",
        None,
        "The module skipped validation[0] because the expression evaluated to an error (failurePolicy is Ignore): no such key: 'count'"
    )]
    #[case::validation_with_params(
        CelError::new("divide by zero"),
        "validation[2]",
        Some(cm(json!({ "namespace": "default", "name": "limits" }))),
        "The module skipped validation[2] for params default/limits because the expression evaluated to an error (failurePolicy is Ignore): divide by zero"
    )]
    #[case::match_condition_without_params_skips_policy(
        CelError::new("divide by zero"),
        "matchCondition 'broken'",
        None,
        "The module skipped the policy because matchCondition 'broken' evaluated to an error (failurePolicy is Ignore): divide by zero"
    )]
    #[case::match_condition_with_params_skips_param(
        CelError::new("divide by zero"),
        "matchCondition 'broken'",
        Some(cm(json!({ "name": "limits" }))),
        "The module skipped params limits because matchCondition 'broken' evaluated to an error (failurePolicy is Ignore): divide by zero"
    )]
    #[case::extension_error_keeps_its_message(
        CelError::from_extension("forbidden", Some("kw.k8s"), "get"),
        "validation[0]",
        None,
        "The module skipped validation[0] because the expression evaluated to an error (failurePolicy is Ignore): forbidden"
    )]
    fn skip_warning_is_built(
        #[case] err: CelError,
        #[case] label: &str,
        #[case] params: Option<CelValue>,
        #[case] expected: &str,
    ) {
        assert_eq!(skip_warning(&err, label, params.as_ref()), expected);
    }
}
