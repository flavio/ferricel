//! Helpers for resolving extension declarations from CLI arguments.

use std::path::Path;

use anyhow::Context;
use ferricel_types::extensions::ExtensionDecl;

/// Parse a single `--extensions` spec string into an [`ExtensionDecl`].
///
/// Expected format: `[namespace.]function:style:arity`
///
/// - `style` is one of `global`, `receiver`, or `both`
/// - `arity` is a positive integer
///
/// # Examples
///
/// ```text
/// "abs:global:1"          -> namespace=None, function="abs", global_style=true,  receiver_style=false, num_args=1
/// "math.sqrt:global:1"    -> namespace=Some("math"), function="sqrt", ...
/// "reverse:receiver:1"    -> global_style=false, receiver_style=true
/// "greet:both:2"          -> global_style=true, receiver_style=true
/// ```
pub fn parse_extension_spec(spec: &str) -> Result<ExtensionDecl, anyhow::Error> {
    // Split from the right to extract arity, then style, then name_part.
    // We split on ':' expecting exactly 3 colon-separated segments.
    let parts: Vec<&str> = spec.splitn(3, ':').collect();
    if parts.len() != 3 {
        anyhow::bail!(
            "invalid extension spec {:?}: expected format [namespace.]function:style:arity \
             (e.g. \"abs:global:1\" or \"math.abs:global:1\")",
            spec
        );
    }

    let name_part = parts[0];
    let style_part = parts[1];
    let arity_part = parts[2];

    // Parse arity — must be a non-negative integer.
    let num_args: usize = arity_part.parse().with_context(|| {
        format!(
            "invalid arity in extension spec {:?}: {:?} is not a valid non-negative integer",
            spec, arity_part
        )
    })?;

    // Parse style.
    let (global_style, receiver_style) = match style_part {
        "global" => (true, false),
        "receiver" => (false, true),
        "both" => (true, true),
        other => anyhow::bail!(
            "invalid style in extension spec {:?}: {:?} is not one of \
             \"global\", \"receiver\", or \"both\"",
            spec,
            other
        ),
    };

    // Split name_part on the last dot to obtain optional namespace and function name.
    let (namespace, function) = if let Some(dot_pos) = name_part.rfind('.') {
        let ns = name_part[..dot_pos].to_string();
        let func = name_part[dot_pos + 1..].to_string();
        if func.is_empty() {
            anyhow::bail!(
                "invalid extension spec {:?}: function name must not be empty \
                 (found trailing dot in {:?})",
                spec,
                name_part
            );
        }
        (Some(ns), func)
    } else {
        (None, name_part.to_string())
    };

    if function.is_empty() {
        anyhow::bail!(
            "invalid extension spec {:?}: function name must not be empty",
            spec
        );
    }

    Ok(ExtensionDecl {
        namespace,
        function,
        global_style,
        receiver_style,
        num_args,
    })
}

/// Load extension declarations from a JSON file.
///
/// The file must contain a JSON array of objects matching the [`ExtensionDecl`] shape.
pub fn load_extensions_file(path: &Path) -> Result<Vec<ExtensionDecl>, anyhow::Error> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read extensions file: {}", path.display()))?;

    let decls: Vec<ExtensionDecl> = serde_json::from_str(&content).with_context(|| {
        format!(
            "failed to parse extensions file {:?}: expected a JSON array of extension \
             declaration objects with fields: namespace, function, global_style, \
             receiver_style, num_args",
            path.display()
        )
    })?;

    Ok(decls)
}

/// Resolve the final list of [`ExtensionDecl`]s from the two mutually exclusive CLI inputs.
///
/// - If `specs` is non-empty, parse each spec string.
/// - If `file` is `Some`, load from the JSON file.
/// - If both are empty/None, return an empty vec.
pub fn resolve_extensions(
    specs: Vec<String>,
    file: Option<&Path>,
) -> Result<Vec<ExtensionDecl>, anyhow::Error> {
    if !specs.is_empty() {
        specs.iter().map(|s| parse_extension_spec(s)).collect()
    } else if let Some(path) = file {
        load_extensions_file(path)
    } else {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use tempfile::NamedTempFile;

    use super::*;

    // -------------------------------------------------------------------------
    // Valid specs — happy path
    // -------------------------------------------------------------------------

    #[rstest]
    #[case::global_no_namespace("abs:global:1", None, "abs", true, false, 1)]
    #[case::global_with_namespace("math.sqrt:global:1", Some("math"), "sqrt", true, false, 1)]
    #[case::global_multi_segment_namespace(
        "com.example.pow:global:2",
        Some("com.example"),
        "pow",
        true,
        false,
        2
    )]
    #[case::receiver_no_namespace("reverse:receiver:1", None, "reverse", false, true, 1)]
    #[case::receiver_with_namespace("str.upper:receiver:1", Some("str"), "upper", false, true, 1)]
    #[case::both_styles("greet:both:2", None, "greet", true, true, 2)]
    #[case::both_styles_namespaced("math.clamp:both:3", Some("math"), "clamp", true, true, 3)]
    #[case::zero_arity("ping:global:0", None, "ping", true, false, 0)]
    fn test_parse_valid_spec(
        #[case] spec: &str,
        #[case] expected_namespace: Option<&str>,
        #[case] expected_function: &str,
        #[case] expected_global: bool,
        #[case] expected_receiver: bool,
        #[case] expected_num_args: usize,
    ) {
        let decl = parse_extension_spec(spec).unwrap();
        assert_eq!(decl.namespace.as_deref(), expected_namespace);
        assert_eq!(decl.function, expected_function);
        assert_eq!(decl.global_style, expected_global);
        assert_eq!(decl.receiver_style, expected_receiver);
        assert_eq!(decl.num_args, expected_num_args);
    }

    // -------------------------------------------------------------------------
    // Invalid specs — error path
    // -------------------------------------------------------------------------

    #[rstest]
    #[case::missing_arity_segment("abs:global")]
    #[case::missing_style_and_arity("abs")]
    #[case::invalid_style("abs:unknown:1")]
    #[case::invalid_arity_not_a_number("abs:global:notanumber")]
    #[case::invalid_arity_negative("abs:global:-1")]
    #[case::trailing_dot_in_name("math.:global:1")]
    fn test_parse_invalid_spec(#[case] spec: &str) {
        assert!(
            parse_extension_spec(spec).is_err(),
            "expected error for spec {spec:?}"
        );
    }

    // -------------------------------------------------------------------------
    // resolve_extensions — dispatching logic
    // -------------------------------------------------------------------------

    #[test]
    fn test_resolve_empty_returns_empty_vec() {
        let result = resolve_extensions(vec![], None).unwrap();
        assert!(result.is_empty());
    }

    #[rstest]
    #[case::single_spec(
        vec!["abs:global:1".to_string()],
        1,
        None,
        "abs",
        true,
        false,
        1
    )]
    #[case::multiple_specs(
        vec!["abs:global:1".to_string(), "math.sqrt:global:1".to_string()],
        2,
        Some("math"),
        "sqrt",
        true,
        false,
        1
    )]
    fn test_resolve_from_specs(
        #[case] specs: Vec<String>,
        #[case] expected_len: usize,
        #[case] last_namespace: Option<&str>,
        #[case] last_function: &str,
        #[case] last_global: bool,
        #[case] last_receiver: bool,
        #[case] last_num_args: usize,
    ) {
        let decls = resolve_extensions(specs, None).unwrap();
        assert_eq!(decls.len(), expected_len);
        let last = decls.last().unwrap();
        assert_eq!(last.namespace.as_deref(), last_namespace);
        assert_eq!(last.function, last_function);
        assert_eq!(last.global_style, last_global);
        assert_eq!(last.receiver_style, last_receiver);
        assert_eq!(last.num_args, last_num_args);
    }

    #[rstest]
    #[case::bad_style("abs:unknown:1")]
    #[case::bad_arity("abs:global:notanumber")]
    #[case::missing_segments("abs:global")]
    fn test_resolve_specs_error_propagates(#[case] bad_spec: &str) {
        let result = resolve_extensions(vec![bad_spec.to_string()], None);
        assert!(result.is_err(), "expected error for spec {bad_spec:?}");
    }

    #[test]
    fn test_resolve_from_file() {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            r#"[
                { "namespace": "math", "function": "sqrt",
                  "global_style": true, "receiver_style": false, "num_args": 1 },
                { "namespace": null, "function": "abs",
                  "global_style": true, "receiver_style": false, "num_args": 1 }
            ]"#,
        )
        .unwrap();

        let decls = resolve_extensions(vec![], Some(file.path())).unwrap();
        assert_eq!(decls.len(), 2);
        assert_eq!(decls[0].namespace.as_deref(), Some("math"));
        assert_eq!(decls[0].function, "sqrt");
        assert_eq!(decls[1].namespace, None);
        assert_eq!(decls[1].function, "abs");
    }

    #[test]
    fn test_resolve_file_not_found() {
        let result = resolve_extensions(
            vec![],
            Some(std::path::Path::new(
                "/tmp/nonexistent_extensions_xyz123.json",
            )),
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("failed to read extensions file")
        );
    }

    #[test]
    fn test_resolve_file_invalid_json() {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(file.path(), "this is not json { @@@ }").unwrap();

        let result = resolve_extensions(vec![], Some(file.path()));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("failed to parse extensions file")
        );
    }
}
