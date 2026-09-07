//! The CEL runtime error type.
//!
//! [`CelRuntimeError`] has three roles:
//!
//! - In the Wasm guest, it is the payload of the `CelValue::Error` variant.
//!   The error flows through the expression tree until `&&`, `||`, or `?:`
//!   absorbs it, or until the guest calls `cel_abort`.
//! - On the wire, the guest serializes it to JSON and passes the bytes to the
//!   `cel_abort` host import.
//! - On the host, `Engine::eval` returns it inside an `anyhow::Error`. The
//!   host can get it back with `err.downcast_ref::<CelRuntimeError>()`.
//!
//! The struct is `#[non_exhaustive]`. A later release can add fields without
//! a breaking change. Use [`CelRuntimeError::new`] or
//! [`CelRuntimeError::from_extension`] to construct it.

use serde::{Deserialize, Serialize};

/// The host extension call that produced a [`CelRuntimeError`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExtensionOrigin {
    /// The namespace of the extension, for example `kw.k8s`. `None` for a
    /// global function.
    pub namespace: Option<String>,
    /// The function name, for example `get`.
    pub function: String,
}

impl std::fmt::Display for ExtensionOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.namespace {
            Some(ns) => write!(f, "{}.{}", ns, self.function),
            None => f.write_str(&self.function),
        }
    }
}

/// A CEL runtime error.
///
/// Examples of runtime errors: divide by zero, integer overflow, an unbound
/// variable, a missing map key, or an `Err` value from a host extension.
///
/// The `Display` text is `CEL runtime error: <message>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("CEL runtime error: {message}")]
#[non_exhaustive]
pub struct CelRuntimeError {
    /// The error message, for example `divide by zero`.
    pub message: String,
    /// The host extension call that produced the error. `None` when the
    /// error did not come from a host extension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<ExtensionOrigin>,
}

impl CelRuntimeError {
    /// Create an error with no origin.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            origin: None,
        }
    }

    /// Create an error that came from the host extension `namespace.function`.
    pub fn from_extension(
        message: impl Into<String>,
        namespace: Option<impl Into<String>>,
        function: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            origin: Some(ExtensionOrigin {
                namespace: namespace.map(Into::into),
                function: function.into(),
            }),
        }
    }

    /// Return `true` when a host extension produced this error.
    pub fn is_from_extension(&self) -> bool {
        self.origin.is_some()
    }
}

impl From<String> for CelRuntimeError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for CelRuntimeError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::without_origin(
        CelRuntimeError::new("divide by zero"),
        "CEL runtime error: divide by zero"
    )]
    #[case::with_origin(
        CelRuntimeError::from_extension("boom", Some("kw.k8s"), "get"),
        "CEL runtime error: boom"
    )]
    fn display_has_the_prefix_and_ignores_the_origin(
        #[case] err: CelRuntimeError,
        #[case] expected: &str,
    ) {
        assert_eq!(err.to_string(), expected);
    }

    #[rstest]
    #[case::without_origin(CelRuntimeError::new("divide by zero"))]
    #[case::with_origin(CelRuntimeError::from_extension("not found", Some("kw.k8s"), "get"))]
    fn json_round_trip(#[case] err: CelRuntimeError) {
        let json = serde_json::to_string(&err).unwrap();
        let back: CelRuntimeError = serde_json::from_str(&json).unwrap();
        assert_eq!(back, err);
    }

    #[test]
    fn json_omits_a_missing_origin() {
        let err = CelRuntimeError::new("divide by zero");
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"message":"divide by zero"}"#);

        let back: CelRuntimeError = serde_json::from_str(r#"{"message":"x"}"#).unwrap();
        assert_eq!(back, CelRuntimeError::new("x"));
    }

    #[rstest]
    #[case::without_origin(CelRuntimeError::new("divide by zero"), false)]
    #[case::with_origin(
        CelRuntimeError::from_extension("not found", Some("kw.k8s"), "get"),
        true
    )]
    fn is_from_extension(#[case] err: CelRuntimeError, #[case] expected: bool) {
        assert_eq!(err.is_from_extension(), expected);
    }

    #[rstest]
    #[case::with_namespace(Some("kw.k8s"), "kw.k8s.get")]
    #[case::without_namespace(None, "get")]
    fn origin_display(#[case] namespace: Option<&str>, #[case] expected: &str) {
        let origin = ExtensionOrigin {
            namespace: namespace.map(String::from),
            function: "get".into(),
        };
        assert_eq!(origin.to_string(), expected);
    }

    #[test]
    fn from_string_and_str() {
        let a: CelRuntimeError = "x".into();
        let b: CelRuntimeError = String::from("x").into();
        assert_eq!(a, b);
        assert_eq!(a, CelRuntimeError::new("x"));
    }
}
