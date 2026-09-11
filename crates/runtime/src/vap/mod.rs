//! VAP (ValidatingAdmissionPolicy) runtime support.
//!
//! - [`response`]: the `ValidationResponse` JSON that the compiled module
//!   returns (`{"accepted": true}` or
//!   `{"accepted": false, "message": "...", "code": N}`).
//! - [`params`]: `params` resolution. [`params::cel_vap_resolve_params`]
//!   reads `paramRef` from the bindings, calls the host `kw.k8s.get` or
//!   `kw.k8s.list` extension, and returns the list of param objects that the
//!   policy must evaluate.
//! - [`label_selector`]: formats a Kubernetes `LabelSelector` as the label
//!   selector string that `kw.k8s.list` expects.

use std::collections::HashMap;

use crate::types::{CelMapKey, CelValue};

mod label_selector;
mod params;
mod response;

/// Read a string field from a map. Returns `None` when the field is missing
/// or is not a string.
fn get_str<'a>(map: &'a HashMap<CelMapKey, CelValue>, key: &str) -> Option<&'a str> {
    match map.get(&CelMapKey::from(key)) {
        Some(CelValue::String(s)) => Some(s.as_str()),
        _ => None,
    }
}
