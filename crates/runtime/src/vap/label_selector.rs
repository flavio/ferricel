//! Label selector formatting for the VAP `params` lookup.
//!
//! `paramRef.selector` is a Kubernetes `LabelSelector` object. The host
//! `kw.k8s.list` extension expects a label selector string instead. This
//! module converts one into the other.

use std::collections::HashMap;

use super::get_str;
use crate::{
    error::{CelError, CelResult},
    types::{CelMapKey, CelValue},
};

/// Format a Kubernetes `LabelSelector` as a label selector string.
///
/// The output matches `labels.Selector.String()` from `k8s.io/apimachinery`:
///
/// - `matchLabels` `{k: v}` becomes `k=v`.
/// - `matchExpressions` with operator `In` becomes `k in (a,b)`.
/// - `NotIn` becomes `k notin (a,b)`.
/// - `Exists` becomes `k`.
/// - `DoesNotExist` becomes `!k`.
///
/// Requirements are sorted by key. The values of `In` and `NotIn` are
/// sorted. Requirements are joined with `,`. An empty selector produces `""`,
/// which matches every resource.
///
/// Returns an error for an unknown operator, or for a field with the wrong
/// type.
pub(super) fn format_label_selector(selector: &HashMap<CelMapKey, CelValue>) -> CelResult<String> {
    // (key, formatted requirement)
    let mut requirements: Vec<(String, String)> = Vec::new();

    if let Some(match_labels) = selector.get(&CelMapKey::from("matchLabels")) {
        let CelValue::Object(match_labels) = match_labels else {
            return Err(CelError::new("paramRef.selector.matchLabels must be a map"));
        };
        for (key, value) in match_labels {
            let key = key.to_string_key();
            let CelValue::String(value) = value else {
                return Err(CelError::new(format!(
                    "paramRef.selector.matchLabels[{key}] must be a string"
                )));
            };
            requirements.push((key.clone(), format!("{key}={value}")));
        }
    }

    if let Some(match_expressions) = selector.get(&CelMapKey::from("matchExpressions")) {
        let CelValue::Array(match_expressions) = match_expressions else {
            return Err(CelError::new(
                "paramRef.selector.matchExpressions must be a list",
            ));
        };
        for expression in match_expressions {
            let CelValue::Object(expression) = expression else {
                return Err(CelError::new(
                    "paramRef.selector.matchExpressions items must be maps",
                ));
            };
            let key = get_str(expression, "key").ok_or_else(|| {
                CelError::new("paramRef.selector.matchExpressions item has no `key` string")
            })?;
            let operator = get_str(expression, "operator").ok_or_else(|| {
                CelError::new("paramRef.selector.matchExpressions item has no `operator` string")
            })?;
            let values = match_expression_values(expression)?;

            let formatted = match operator {
                "In" => format!("{key} in ({})", values.join(",")),
                "NotIn" => format!("{key} notin ({})", values.join(",")),
                "Exists" => key.to_string(),
                "DoesNotExist" => format!("!{key}"),
                other => {
                    return Err(CelError::new(format!(
                        "unknown label selector operator: {other}"
                    )));
                }
            };
            requirements.push((key.to_string(), formatted));
        }
    }

    requirements.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(requirements
        .into_iter()
        .map(|(_, formatted)| formatted)
        .collect::<Vec<_>>()
        .join(","))
}

/// Read and sort the `values` of one `matchExpressions` item. A missing
/// `values` field is an empty list.
fn match_expression_values(expression: &HashMap<CelMapKey, CelValue>) -> CelResult<Vec<String>> {
    let mut values = match expression.get(&CelMapKey::from("values")) {
        None | Some(CelValue::Null) => Vec::new(),
        Some(CelValue::Array(values)) => values
            .iter()
            .map(|v| match v {
                CelValue::String(s) => Ok(s.clone()),
                _ => Err(CelError::new(
                    "paramRef.selector.matchExpressions values must be strings",
                )),
            })
            .collect::<CelResult<Vec<_>>>()?,
        Some(_) => {
            return Err(CelError::new(
                "paramRef.selector.matchExpressions values must be a list",
            ));
        }
    };
    values.sort();
    Ok(values)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;

    use super::*;
    use crate::test_helpers::make_json_map;

    /// The `combined` case is the reference case from `cel-policy`
    /// (`internal/validate/namespace_object_test.go`). The input is not
    /// sorted. The output is sorted by key, and `In`/`NotIn` values are
    /// sorted.
    #[rstest]
    #[case::match_labels_only(
        json!({ "matchLabels": { "environment": "test", "app": "web" } }),
        "app=web,environment=test"
    )]
    #[case::in_sorts_values(
        json!({ "matchExpressions": [
            { "key": "env", "operator": "In", "values": ["staging", "production"] }
        ] }),
        "env in (production,staging)"
    )]
    #[case::notin(
        json!({ "matchExpressions": [
            { "key": "phase", "operator": "NotIn", "values": ["initial", "final"] }
        ] }),
        "phase notin (final,initial)"
    )]
    #[case::exists(
        json!({ "matchExpressions": [{ "key": "foo", "operator": "Exists" }] }),
        "foo"
    )]
    #[case::does_not_exist(
        json!({ "matchExpressions": [{ "key": "bar", "operator": "DoesNotExist", "values": [] }] }),
        "!bar"
    )]
    #[case::empty_matches_everything(json!({}), "")]
    #[case::combined(
        json!({
            "matchLabels": { "app": "my-app" },
            "matchExpressions": [
                { "key": "environment", "operator": "In", "values": ["production", "staging"] },
                { "key": "phase", "operator": "NotIn", "values": ["initial", "final"] },
                { "key": "foo", "operator": "Exists", "values": [] },
                { "key": "bar", "operator": "DoesNotExist", "values": [] }
            ]
        }),
        "app=my-app,!bar,environment in (production,staging),foo,phase notin (final,initial)"
    )]
    fn selector_is_formatted_and_sorted(
        #[case] selector: serde_json::Value,
        #[case] expected: &str,
    ) {
        assert_eq!(
            format_label_selector(&make_json_map(selector)).unwrap(),
            expected
        );
    }

    #[rstest]
    #[case::unknown_operator(
        json!({ "matchExpressions": [{ "key": "foo", "operator": "Equals", "values": ["x"] }] }),
        "unknown label selector operator: Equals"
    )]
    #[case::match_labels_not_a_map(
        json!({ "matchLabels": "app=web" }),
        "paramRef.selector.matchLabels must be a map"
    )]
    #[case::match_label_value_not_a_string(
        json!({ "matchLabels": { "replicas": 3 } }),
        "paramRef.selector.matchLabels[replicas] must be a string"
    )]
    #[case::match_expressions_not_a_list(
        json!({ "matchExpressions": {} }),
        "paramRef.selector.matchExpressions must be a list"
    )]
    #[case::match_expression_not_a_map(
        json!({ "matchExpressions": ["foo"] }),
        "paramRef.selector.matchExpressions items must be maps"
    )]
    #[case::match_expression_without_key(
        json!({ "matchExpressions": [{ "operator": "Exists" }] }),
        "paramRef.selector.matchExpressions item has no `key` string"
    )]
    #[case::match_expression_without_operator(
        json!({ "matchExpressions": [{ "key": "foo" }] }),
        "paramRef.selector.matchExpressions item has no `operator` string"
    )]
    #[case::values_not_a_list(
        json!({ "matchExpressions": [{ "key": "foo", "operator": "In", "values": "a" }] }),
        "paramRef.selector.matchExpressions values must be a list"
    )]
    #[case::values_not_strings(
        json!({ "matchExpressions": [{ "key": "foo", "operator": "In", "values": [1] }] }),
        "paramRef.selector.matchExpressions values must be strings"
    )]
    fn selector_invalid_input_is_an_error(
        #[case] selector: serde_json::Value,
        #[case] message: &str,
    ) {
        let err = format_label_selector(&make_json_map(selector)).unwrap_err();
        assert_eq!(err.message, message);
        assert_eq!(err.origin, None);
    }
}
