//! VAP `namespaceObject` resolution.
//!
//! [`cel_vap_resolve_namespace_object`] reads `request` from the bindings
//! and, when the request is namespaced, calls the host `kw.k8s.get`
//! extension to fetch the Namespace of the resource under admission.

use super::{FetchFn, base_request_map, get_str, read_binding};
use crate::{
    error::{CelError, CelResult, into_raw_result},
    extensions::call_extension_impl,
    types::CelValue,
};

/// Resolve `namespaceObject` for a VAP evaluation.
///
/// The function reads `request` from the bindings, then decides:
///
/// - `request` is missing, or not a map: `CelValue::Error`. The host
///   contract requires `request` whenever the policy references
///   `namespaceObject`.
/// - `request.kind` is `{group: "", version: "v1", kind: "Namespace"}`: the
///   resource under admission is itself a Namespace. `namespaceObject` is
///   `CelValue::Null`, and no host call is made. This matches the
///   Kubernetes special case: a Namespace request carries its own name as
///   `request.namespace`, and the Namespace does not exist yet on `CREATE`.
/// - `request.namespace` is missing or empty: the request is cluster-scoped.
///   `namespaceObject` is `CelValue::Null`, and no host call is made.
/// - Otherwise: one `kw.k8s.get` call for
///   `{apiVersion: "v1", kind: "Namespace", name: request.namespace}`. The
///   request map has no `namespace` key, because `Namespace` is
///   cluster-scoped. A host error keeps its `kw.k8s.get` origin.
///
/// # Returns
/// A heap-allocated `CelValue::Object` (the Namespace), `CelValue::Null`, or
/// a `CelValue::Error`.
///
/// # Safety
/// `cel_init_bindings` must have run before this function.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_vap_resolve_namespace_object() -> *mut CelValue {
    let request = read_binding("request");

    let mut fetch = |function: &str, request_map: CelValue| -> CelValue {
        let ptr = call_extension_impl(Some("kw.k8s"), function, vec![request_map]);
        // `call_extension_impl` never returns null.
        (*ptr).clone()
    };

    into_raw_result(resolve_namespace_object(request.as_ref(), &mut fetch))
}

/// Pure implementation of [`cel_vap_resolve_namespace_object`]. See that
/// function for the semantics. `fetch` performs the host call.
fn resolve_namespace_object(
    request: Option<&CelValue>,
    fetch: &mut FetchFn<'_>,
) -> CelResult<CelValue> {
    let request = match request {
        None => return Err(CelError::new("request binding is missing")),
        Some(CelValue::Object(map)) => map,
        Some(_) => return Err(CelError::new("request binding must be a map")),
    };

    if is_namespace_kind(request) {
        return Ok(CelValue::Null);
    }

    let namespace = get_str(request, "namespace").filter(|s| !s.is_empty());
    let Some(namespace) = namespace else {
        return Ok(CelValue::Null);
    };

    let mut request_map = base_request_map("v1", "Namespace", None);
    request_map.insert(
        crate::types::CelMapKey::from("name"),
        CelValue::String(namespace.to_string()),
    );

    match fetch("get", CelValue::Object(request_map)) {
        CelValue::Error(err) => Err(err),
        value => Ok(value),
    }
}

/// Is `request.kind` the core `v1/Namespace` GroupVersionKind?
///
/// `request.kind` is `{group, version, kind}`, mirroring the Kubernetes
/// `AdmissionRequest.Kind` field.
fn is_namespace_kind(
    request: &std::collections::HashMap<crate::types::CelMapKey, CelValue>,
) -> bool {
    let Some(CelValue::Object(kind)) = request.get(&crate::types::CelMapKey::from("kind")) else {
        return false;
    };
    get_str(kind, "group") == Some("")
        && get_str(kind, "version") == Some("v1")
        && get_str(kind, "kind") == Some("Namespace")
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;

    use super::*;
    use crate::test_helpers::make_json;

    /// A host error with the `kw.k8s.get` origin, as `call_extension_impl`
    /// produces it.
    fn host_error(message: &str) -> CelValue {
        CelValue::Error(CelError::from_extension(message, Some("kw.k8s"), "get"))
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
    }

    fn resolve(request: Option<serde_json::Value>, host: &mut MockHost) -> CelResult<CelValue> {
        let request = request.map(make_json);
        resolve_namespace_object(request.as_ref(), &mut host.fetch())
    }

    fn namespace_obj(name: &str) -> serde_json::Value {
        json!({ "kind": "Namespace", "metadata": { "name": name } })
    }

    #[rstest]
    #[case::missing(None, "request binding is missing")]
    #[case::not_a_map(Some(json!("not a map")), "request binding must be a map")]
    fn malformed_request_is_an_error_without_a_call(
        #[case] request: Option<serde_json::Value>,
        #[case] message: &str,
    ) {
        let mut host = MockHost::returning(make_json(namespace_obj("team-a")));
        let err = resolve(request, &mut host).unwrap_err();
        assert_eq!(err.message, message);
        assert!(host.calls.is_empty(), "the host must not be called");
    }

    #[rstest]
    #[case::namespace_missing(json!({}))]
    #[case::namespace_empty(json!({ "namespace": "" }))]
    #[case::namespace_kind(json!({
        "namespace": "team-a",
        "kind": { "group": "", "version": "v1", "kind": "Namespace" }
    }))]
    fn request_without_a_namespace_to_fetch_is_null_without_a_call(
        #[case] request: serde_json::Value,
    ) {
        let mut host = MockHost::returning(make_json(namespace_obj("team-a")));
        let result = resolve(Some(request), &mut host).unwrap();
        assert_eq!(result, CelValue::Null);
        assert!(host.calls.is_empty(), "the host must not be called");
    }

    #[test]
    fn namespaced_request_calls_get_with_the_expected_map() {
        let mut host = MockHost::returning(make_json(namespace_obj("team-a")));
        let result = resolve(Some(json!({ "namespace": "team-a" })), &mut host).unwrap();

        assert_eq!(result, make_json(namespace_obj("team-a")));
        assert_eq!(host.calls.len(), 1);
        assert_eq!(host.calls[0].0, "get");
        let request = match &host.calls[0].1 {
            CelValue::Object(map) => map,
            other => panic!("request is not a map: {other:?}"),
        };
        assert_eq!(get_str(request, "__type__"), Some("kw.k8s.Client"));
        assert_eq!(get_str(request, "apiVersion"), Some("v1"));
        assert_eq!(get_str(request, "kind"), Some("Namespace"));
        assert_eq!(get_str(request, "name"), Some("team-a"));
        assert!(
            request
                .get(&crate::types::CelMapKey::from("namespace"))
                .is_none(),
            "the Namespace fetch must not carry a `namespace` key"
        );
    }

    #[test]
    fn host_error_is_returned_as_is() {
        let mut host = MockHost::returning(host_error("namespaces \"team-a\" not found"));
        let err = resolve(Some(json!({ "namespace": "team-a" })), &mut host).unwrap_err();
        assert_eq!(err.message, "namespaces \"team-a\" not found");
        assert_eq!(
            err.origin.as_ref().map(|o| o.function.as_str()),
            Some("get")
        );
    }
}
