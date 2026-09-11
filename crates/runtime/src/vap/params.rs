//! VAP `params` resolution.
//!
//! [`cel_vap_resolve_params`] reads `paramRef` and `request` from the
//! bindings, calls the host `kw.k8s.get` or `kw.k8s.list` extension, and
//! returns the list of param objects that the policy must evaluate once each.

use std::collections::HashMap;

use super::{get_str, label_selector::format_label_selector};
use crate::{
    error::{CelError, CelResult, into_raw_result},
    extensions::call_extension_impl,
    globals::cel_get_variable,
    types::{CelMapKey, CelValue},
};

/// The `__type__` tag of the request map that `kw.k8s.get` and `kw.k8s.list`
/// receive. It is the same tag that `cel_builder_step` writes for a
/// `kw.k8s.apiVersion(...).kind(...)` chain.
const KW_K8S_CLIENT_TYPE: &str = "kw.k8s.Client";

/// Resolve the list of `params` objects for a VAP evaluation.
///
/// The function reads `paramRef` and `request` from the bindings. Then it
/// fetches the param resources from the host:
///
/// - `paramRef.name` set: one `kw.k8s.get` call. The result is a list with
///   one item.
/// - `paramRef.selector` set: one `kw.k8s.list` call with the selector
///   formatted as a label selector string. The result is the `items` list.
///
/// The `namespace` in the request map is `paramRef.namespace`. If that is
/// empty, it is `request.namespace`. If that is also empty, it is `""`.
///
/// When the host call fails, or the list is empty, the value of
/// `paramRef.parameterNotFoundAction` decides the result:
///
/// - `"Allow"`: the function returns an empty list. The policy then accepts
///   the request.
/// - Any other value, or no value: the function returns a `CelValue::Error`.
///   An error from the host keeps its `kw.k8s.get` or `kw.k8s.list` origin.
///   An empty list produces the message `no parameters found`.
///
/// A malformed `paramRef` (not a map, no `name` and no `selector`, bad
/// selector operator) is always an error. `parameterNotFoundAction` does not
/// apply to it.
///
/// # Parameters
/// - `api_version_ptr`, `api_version_len`: UTF-8 bytes of `paramKind.apiVersion`.
/// - `kind_ptr`, `kind_len`: UTF-8 bytes of `paramKind.kind`.
///
/// # Returns
/// A heap-allocated `CelValue::Array` of param objects, or a `CelValue::Error`.
///
/// # Safety
/// - Both pointer/length pairs must describe valid UTF-8 bytes in Wasm memory.
/// - `cel_init_bindings` must have run before this function.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_vap_resolve_params(
    api_version_ptr: *const u8,
    api_version_len: i32,
    kind_ptr: *const u8,
    kind_len: i32,
) -> *mut CelValue {
    let api_version = read_str(api_version_ptr, api_version_len);
    let kind = read_str(kind_ptr, kind_len);

    let param_ref = read_binding("paramRef");
    let request = read_binding("request");

    let mut fetch = |function: &str, request_map: CelValue| -> CelValue {
        let ptr = call_extension_impl(Some("kw.k8s"), function, vec![request_map]);
        // `call_extension_impl` never returns null.
        (*ptr).clone()
    };

    into_raw_result(resolve_params(
        api_version,
        kind,
        param_ref.as_ref(),
        request.as_ref(),
        &mut fetch,
    ))
}

/// Read a `&str` from a raw pointer and length. A zero length gives `""`
/// without a memory read, because the compiler passes a null pointer for an
/// empty string.
///
/// # Safety
/// When `len > 0`, `ptr` must point to `len` valid UTF-8 bytes.
unsafe fn read_str<'a>(ptr: *const u8, len: i32) -> &'a str {
    if len <= 0 {
        return "";
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    std::str::from_utf8(bytes).unwrap_or_else(|_| {
        crate::error::abort_with_error("cel_vap_resolve_params: argument is not valid UTF-8")
    })
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

/// A host call that fetches params. The first argument is the `kw.k8s`
/// function name (`get` or `list`). The second argument is the request map.
/// The result is the host response, or a `CelValue::Error`.
type FetchFn<'a> = dyn FnMut(&str, CelValue) -> CelValue + 'a;

/// Where the params come from.
enum ParamsSource<'a> {
    /// `paramRef.name`: one resource.
    Name(&'a str),
    /// `paramRef.selector`: every resource that matches the label selector.
    Selector(&'a HashMap<CelMapKey, CelValue>),
}

/// The result of one fetch, before `parameterNotFoundAction` is applied.
enum FetchOutcome {
    /// The host returned these param objects. The list can be empty.
    Found(Vec<CelValue>),
    /// The host call failed. Under `Allow` this counts as "not found".
    HostError(CelError),
}

/// Pure implementation of [`cel_vap_resolve_params`]. See that function for
/// the semantics. `fetch` performs the host call.
fn resolve_params(
    api_version: &str,
    kind: &str,
    param_ref: Option<&CelValue>,
    request: Option<&CelValue>,
    fetch: &mut FetchFn<'_>,
) -> CelResult<CelValue> {
    let param_ref = match param_ref {
        None => return Err(CelError::new("paramRef binding is missing")),
        Some(CelValue::Object(map)) => map,
        Some(_) => return Err(CelError::new("paramRef binding must be a map")),
    };

    let namespace = resolve_namespace(param_ref, request);
    let allow_not_found = get_str(param_ref, "parameterNotFoundAction") == Some("Allow");

    let outcome = match params_source(param_ref)? {
        ParamsSource::Name(name) => {
            let mut request_map = base_request_map(api_version, kind, &namespace);
            request_map.insert(CelMapKey::from("name"), CelValue::String(name.to_string()));
            match fetch("get", CelValue::Object(request_map)) {
                CelValue::Error(err) => FetchOutcome::HostError(err),
                value => FetchOutcome::Found(vec![value]),
            }
        }
        ParamsSource::Selector(selector) => {
            let label_selector = format_label_selector(selector)?;
            let mut request_map = base_request_map(api_version, kind, &namespace);
            request_map.insert(
                CelMapKey::from("labelSelector"),
                CelValue::String(label_selector),
            );
            match fetch("list", CelValue::Object(request_map)) {
                CelValue::Error(err) => FetchOutcome::HostError(err),
                CelValue::Object(mut response) => {
                    match response.remove(&CelMapKey::from("items")) {
                        Some(CelValue::Array(items)) => FetchOutcome::Found(items),
                        _ => {
                            return Err(CelError::new("kw.k8s.list result has no `items` list"));
                        }
                    }
                }
                _ => return Err(CelError::new("kw.k8s.list result is not a map")),
            }
        }
    };

    // "Not found" is: the host call failed, or the list is empty.
    //
    // A host error can be a real 404 or an authorization error. The host
    // does not tell them apart today, so under `Allow` both accept the
    // request. `cel-policy` has the same behavior.
    match outcome {
        FetchOutcome::Found(items) if !items.is_empty() => Ok(CelValue::Array(items)),
        FetchOutcome::Found(_) if allow_not_found => Ok(CelValue::Array(vec![])),
        FetchOutcome::Found(_) => Err(CelError::new("no parameters found")),
        FetchOutcome::HostError(_) if allow_not_found => Ok(CelValue::Array(vec![])),
        FetchOutcome::HostError(err) => Err(err),
    }
}

/// Pick `name` or `selector` from `paramRef`.
fn params_source(param_ref: &HashMap<CelMapKey, CelValue>) -> CelResult<ParamsSource<'_>> {
    if let Some(name) = get_str(param_ref, "name").filter(|s| !s.is_empty()) {
        return Ok(ParamsSource::Name(name));
    }
    if let Some(CelValue::Object(selector)) = param_ref.get(&CelMapKey::from("selector")) {
        return Ok(ParamsSource::Selector(selector));
    }
    Err(CelError::new("paramRef must have either name or selector"))
}

/// Resolve the namespace of the param resources.
///
/// `paramRef.namespace` wins. If it is empty, the namespace of the request
/// (`request.namespace`) is used. If that is also empty, the result is `""`.
/// This is the Kubernetes behavior for per-namespace parameters.
fn resolve_namespace(
    param_ref: &HashMap<CelMapKey, CelValue>,
    request: Option<&CelValue>,
) -> String {
    if let Some(ns) = get_str(param_ref, "namespace").filter(|s| !s.is_empty()) {
        return ns.to_string();
    }
    if let Some(CelValue::Object(request)) = request
        && let Some(ns) = get_str(request, "namespace")
    {
        return ns.to_string();
    }
    String::new()
}

/// Build the request map that both `kw.k8s.get` and `kw.k8s.list` share.
///
/// `namespace` is always present, possibly `""`. This differs from a
/// `kw.k8s` chain written in CEL, where `namespace` is present only when the
/// policy calls `.namespace()`.
fn base_request_map(
    api_version: &str,
    kind: &str,
    namespace: &str,
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
    map.insert(
        CelMapKey::from("namespace"),
        CelValue::String(namespace.to_string()),
    );
    map
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;

    use super::*;
    use crate::test_helpers::{make_json, make_json_map};

    // ─── helpers for the params tests ────────────────────────────────────────

    /// A host error with the `kw.k8s.<function>` origin, as
    /// `call_extension_impl` produces it.
    fn host_error(function: &str, message: &str) -> CelValue {
        CelValue::Error(CelError::from_extension(message, Some("kw.k8s"), function))
    }

    /// Record every host call and return a fixed response.
    struct MockHost {
        calls: Vec<(String, CelValue)>,
        response: CelValue,
    }

    impl MockHost {
        fn returning(response: CelValue) -> Self {
            Self {
                calls: Vec::new(),
                response,
            }
        }

        fn fetch(&mut self) -> impl FnMut(&str, CelValue) -> CelValue + '_ {
            |function, request| {
                self.calls.push((function.to_string(), request));
                self.response.clone()
            }
        }

        /// The request map of the only call.
        fn only_request(&self) -> &HashMap<CelMapKey, CelValue> {
            assert_eq!(self.calls.len(), 1, "expected exactly one host call");
            match &self.calls[0].1 {
                CelValue::Object(map) => map,
                other => panic!("request is not a map: {other:?}"),
            }
        }
    }

    fn resolve(
        param_ref: Option<serde_json::Value>,
        request: Option<serde_json::Value>,
        host: &mut MockHost,
    ) -> CelResult<CelValue> {
        let param_ref = param_ref.map(make_json);
        let request = request.map(make_json);
        resolve_params(
            "v1",
            "ConfigMap",
            param_ref.as_ref(),
            request.as_ref(),
            &mut host.fetch(),
        )
    }

    fn configmap(name: &str) -> serde_json::Value {
        serde_json::json!({ "kind": "ConfigMap", "metadata": { "name": name } })
    }

    // ─── resolve_namespace ───────────────────────────────────────────────────

    #[rstest]
    #[case::param_ref_wins(
        json!({ "namespace": "config" }),
        Some(json!({ "namespace": "default" })),
        "config"
    )]
    #[case::empty_falls_back_to_request(
        json!({ "namespace": "" }),
        Some(json!({ "namespace": "default" })),
        "default"
    )]
    #[case::missing_falls_back_to_request(
        json!({}),
        Some(json!({ "namespace": "default" })),
        "default"
    )]
    #[case::both_empty(json!({}), Some(json!({})), "")]
    #[case::request_absent(json!({}), None, "")]
    fn namespace_is_resolved(
        #[case] param_ref: serde_json::Value,
        #[case] request: Option<serde_json::Value>,
        #[case] expected: &str,
    ) {
        let request = request.map(make_json);
        assert_eq!(
            resolve_namespace(&make_json_map(param_ref), request.as_ref()),
            expected
        );
    }

    // ─── resolve_params ──────────────────────────────────────────────────────

    #[test]
    fn params_by_name_calls_get_and_returns_one_item() {
        let mut host = MockHost::returning(make_json(configmap("cfg")));
        let result = resolve(
            Some(json!({ "name": "cfg", "namespace": "default" })),
            None,
            &mut host,
        )
        .unwrap();

        assert_eq!(result, CelValue::Array(vec![make_json(configmap("cfg"))]));
        assert_eq!(host.calls[0].0, "get");
        let request = host.only_request();
        assert_eq!(get_str(request, "__type__"), Some("kw.k8s.Client"));
        assert_eq!(get_str(request, "apiVersion"), Some("v1"));
        assert_eq!(get_str(request, "kind"), Some("ConfigMap"));
        assert_eq!(get_str(request, "namespace"), Some("default"));
        assert_eq!(get_str(request, "name"), Some("cfg"));
        assert!(request.get(&CelMapKey::from("labelSelector")).is_none());
    }

    #[test]
    fn params_by_selector_calls_list_with_formatted_selector() {
        let mut host = MockHost::returning(make_json(json!({
            "items": [configmap("a"), configmap("b")]
        })));
        let result = resolve(
            Some(json!({
                "namespace": "default",
                "selector": { "matchLabels": { "environment": "test" } }
            })),
            None,
            &mut host,
        )
        .unwrap();

        assert_eq!(
            result,
            CelValue::Array(vec![make_json(configmap("a")), make_json(configmap("b"))])
        );
        assert_eq!(host.calls[0].0, "list");
        let request = host.only_request();
        assert_eq!(get_str(request, "labelSelector"), Some("environment=test"));
        assert_eq!(get_str(request, "namespace"), Some("default"));
        assert!(request.get(&CelMapKey::from("name")).is_none());
    }

    /// What `resolve_params` returns when the params are not found.
    #[derive(Debug)]
    enum NotFoundOutcome {
        /// `parameterNotFoundAction: Allow`: an empty list.
        EmptyList,
        /// Otherwise: an error. `origin` is the `kw.k8s` function name when
        /// the error came from the host.
        Error {
            message: &'static str,
            origin: Option<&'static str>,
        },
    }

    /// "Not found" is a host error or an empty `list` result. Under `Allow`
    /// both give an empty list. Otherwise a host error keeps its origin, and
    /// an empty list becomes `no parameters found`.
    #[rstest]
    #[case::name_host_error_deny(
        json!({ "name": "cfg" }),
        host_error("get", "not found"),
        NotFoundOutcome::Error { message: "not found", origin: Some("get") }
    )]
    #[case::name_host_error_allow(
        json!({ "name": "cfg", "parameterNotFoundAction": "Allow" }),
        host_error("get", "not found"),
        NotFoundOutcome::EmptyList
    )]
    #[case::selector_empty_list_deny(
        json!({ "selector": {} }),
        make_json(json!({ "items": [] })),
        NotFoundOutcome::Error { message: "no parameters found", origin: None }
    )]
    #[case::selector_empty_list_allow(
        json!({ "selector": {}, "parameterNotFoundAction": "Allow" }),
        make_json(json!({ "items": [] })),
        NotFoundOutcome::EmptyList
    )]
    #[case::selector_host_error_deny(
        json!({ "selector": {} }),
        host_error("list", "forbidden"),
        NotFoundOutcome::Error { message: "forbidden", origin: Some("list") }
    )]
    #[case::selector_host_error_allow(
        json!({ "selector": {}, "parameterNotFoundAction": "Allow" }),
        host_error("list", "forbidden"),
        NotFoundOutcome::EmptyList
    )]
    fn params_not_found_honors_parameter_not_found_action(
        #[case] param_ref: serde_json::Value,
        #[case] response: CelValue,
        #[case] expected: NotFoundOutcome,
    ) {
        let mut host = MockHost::returning(response);
        let result = resolve(Some(param_ref), None, &mut host);
        match expected {
            NotFoundOutcome::EmptyList => {
                assert_eq!(result.unwrap(), CelValue::Array(vec![]));
            }
            NotFoundOutcome::Error { message, origin } => {
                let err = result.unwrap_err();
                assert_eq!(err.message, message);
                assert_eq!(
                    err.origin.as_ref().map(|o| o.function.as_str()),
                    origin,
                    "unexpected origin in {err:?}"
                );
            }
        }
    }

    /// A `list` response without `items` is malformed. It is an error even
    /// under `Allow`.
    #[test]
    fn params_by_selector_response_without_items_is_an_error() {
        let mut host = MockHost::returning(make_json(json!({ "kind": "List" })));
        let err = resolve(
            Some(json!({ "selector": {}, "parameterNotFoundAction": "Allow" })),
            None,
            &mut host,
        )
        .unwrap_err();
        assert_eq!(err.message, "kw.k8s.list result has no `items` list");
    }

    /// The `namespace` the host receives, through the full `resolve_params`
    /// path.
    #[rstest]
    #[case::defaults_to_request_namespace(Some(json!({ "namespace": "team-a" })), "team-a")]
    #[case::empty_string_without_request(None, "")]
    fn params_request_carries_resolved_namespace(
        #[case] request: Option<serde_json::Value>,
        #[case] expected: &str,
    ) {
        let mut host = MockHost::returning(make_json(configmap("cfg")));
        resolve(Some(json!({ "name": "cfg" })), request, &mut host).unwrap();
        assert_eq!(get_str(host.only_request(), "namespace"), Some(expected));
    }

    #[test]
    fn params_name_takes_precedence_over_selector() {
        let mut host = MockHost::returning(make_json(configmap("cfg")));
        resolve(
            Some(json!({ "name": "cfg", "selector": {} })),
            None,
            &mut host,
        )
        .unwrap();
        assert_eq!(host.calls[0].0, "get");
    }

    /// A malformed `paramRef` is an error before any host call, also under
    /// `Allow`.
    #[rstest]
    #[case::missing(None, "paramRef binding is missing")]
    #[case::not_a_map(Some(json!("cfg")), "paramRef binding must be a map")]
    #[case::without_name_or_selector(
        Some(json!({ "namespace": "default", "parameterNotFoundAction": "Allow" })),
        "paramRef must have either name or selector"
    )]
    fn params_malformed_param_ref_is_an_error(
        #[case] param_ref: Option<serde_json::Value>,
        #[case] message: &str,
    ) {
        let mut host = MockHost::returning(make_json(configmap("cfg")));
        let err = resolve(param_ref, None, &mut host).unwrap_err();
        assert_eq!(err.message, message);
        assert!(host.calls.is_empty(), "the host must not be called");
    }
}
