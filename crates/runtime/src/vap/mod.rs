//! VAP (ValidatingAdmissionPolicy) runtime support.
//!
//! - [`response`]: the `ValidationResponse` JSON that the compiled module
//!   returns (`{"accepted": true}` or
//!   `{"accepted": false, "message": "...", "code": N}`). Both carry a
//!   `warnings` list when the module skipped an expression under
//!   `failurePolicy: Ignore`.
//! - [`failure_policy`]: `failurePolicy` support. The host passes the policy
//!   in the `failurePolicy` binding (`"Fail"`, the default, or `"Ignore"`).
//!   [`failure_policy::cel_vap_reset`] reads it once per evaluation.
//!   [`failure_policy::cel_vap_expression_errored`] decides, for one
//!   `matchConditions` or `validations` result, whether to trap (`Fail`) or
//!   to skip the expression and record a warning (`Ignore`).
//! - [`params`]: `params` resolution. [`params::cel_vap_resolve_params`]
//!   reads `paramRef` from the bindings, calls the host `kw.k8s.get` or
//!   `kw.k8s.list` extension, and returns the list of param objects that the
//!   policy must evaluate.
//! - [`namespace_object`]: `namespaceObject` resolution.
//!   [`namespace_object::cel_vap_resolve_namespace_object`] reads `request`
//!   from the bindings and calls the host `kw.k8s.get` extension for the
//!   Namespace of the resource under admission.
//! - [`label_selector`]: formats a Kubernetes `LabelSelector` as the label
//!   selector string that `kw.k8s.list` expects.

use std::collections::HashMap;

use crate::{
    globals::cel_get_variable,
    types::{CelMapKey, CelValue},
};

mod failure_policy;
mod label_selector;
mod namespace_object;
mod params;
mod response;

/// The `__type__` tag of the request map that `kw.k8s.get` and `kw.k8s.list`
/// receive. It is the same tag that `cel_builder_step` writes for a
/// `kw.k8s.apiVersion(...).kind(...)` chain.
const KW_K8S_CLIENT_TYPE: &str = "kw.k8s.Client";

/// A host call that performs a `kw.k8s` fetch. The first argument is the
/// `kw.k8s` function name (`get` or `list`). The second argument is the
/// request map. The result is the host response, or a `CelValue::Error`.
type FetchFn<'a> = dyn FnMut(&str, CelValue) -> CelValue + 'a;

/// Read a string field from a map. Returns `None` when the field is missing
/// or is not a string.
fn get_str<'a>(map: &'a HashMap<CelMapKey, CelValue>, key: &str) -> Option<&'a str> {
    match map.get(&CelMapKey::from(key)) {
        Some(CelValue::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// Read one binding by name. Returns `None` when the binding is not set.
///
/// # Safety
/// `cel_init_bindings` must have run before this function.
unsafe fn read_binding(name: &str) -> Option<CelValue> {
    let ptr = unsafe { cel_get_variable(name.as_ptr(), name.len() as i32) };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { (*ptr).clone() })
    }
}

/// Build the request map that `kw.k8s.get` and `kw.k8s.list` share.
///
/// `namespace` is inserted only when `Some`, possibly `""`. This differs
/// from a `kw.k8s` chain written in CEL, where `namespace` is present only
/// when the policy calls `.namespace()`. Callers that always know the
/// namespace (like `params`) pass `Some(&namespace)`; callers that must omit
/// the key for a cluster-scoped resource (like `namespaceObject`) pass
/// `None`.
fn base_request_map(
    api_version: &str,
    kind: &str,
    namespace: Option<&str>,
) -> HashMap<CelMapKey, CelValue> {
    let mut map = HashMap::new();
    map.insert(
        CelMapKey::from("__type__"),
        CelValue::String(KW_K8S_CLIENT_TYPE.to_string()),
    );
    map.insert(
        CelMapKey::from("apiVersion"),
        CelValue::String(api_version.to_string()),
    );
    map.insert(CelMapKey::from("kind"), CelValue::String(kind.to_string()));
    if let Some(namespace) = namespace {
        map.insert(
            CelMapKey::from("namespace"),
            CelValue::String(namespace.to_string()),
        );
    }
    map
}
