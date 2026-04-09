//! Kubernetes CEL URL library extensions.
//!
//! Implements the Kubernetes URL functions described in:
//!   <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-url-library>
//!
//! Functions:
//!   - `url(string)`         → URL (or error if invalid/relative)
//!   - `isURL(string)`       → bool
//!   - `<URL>.getScheme()`   → string
//!   - `<URL>.getHost()`     → string  (host:port, IPv6 with brackets)
//!   - `<URL>.getHostname()` → string  (host only, IPv6 without brackets)
//!   - `<URL>.getPort()`     → string
//!   - `<URL>.getEscapedPath()` → string (percent-encoded)
//!   - `<URL>.getQuery()`    → map\<string, list\<string\>>

use crate::error::create_error_value;
use crate::types::{CelMapKey, CelValue};
use slog::error;
use std::collections::HashMap;
use url::{Host, Url};

// ─────────────────────────────────────────────────────────────────────────────
// Validation helper
// ─────────────────────────────────────────────────────────────────────────────

/// Validates a string as an absolute URI or absolute path.
///
/// Mirrors Go's `url.ParseRequestURI`: accepts absolute URIs (have a scheme)
/// and absolute paths (start with `/`). Rejects relative paths.
///
/// Returns `Ok((Url, original_string))` on success, `Err(message)` on failure.
/// The original string is stored alongside the parsed URL so that accessors can
/// distinguish a normalised implicit path `"/"` from an explicit one.
fn is_valid_request_uri(s: &str) -> Result<(Url, String), String> {
    // Handle absolute paths: url::Url::parse fails for paths like "/foo",
    // so we fabricate a synthetic base URL to parse them.
    if s.starts_with('/') {
        let synthetic = format!("https://localhost{}", s);
        let u = Url::parse(&synthetic)
            .map_err(|e| format!("URL parse error for absolute path: {}", e))?;
        return Ok((u, s.to_string()));
    }

    // For absolute URIs, parse directly.
    let u = Url::parse(s).map_err(|e| format!("URL parse error: {}", e))?;

    // Reject relative references (url::Url::parse won't produce them, but be safe).
    if u.cannot_be_a_base() {
        return Err(format!("relative URL not allowed: {}", s));
    }

    Ok((u, s.to_string()))
}

/// Returns `true` if the original input was an absolute path (no scheme/host).
fn is_path_only(original: &str) -> bool {
    original.starts_with('/')
}

// ─────────────────────────────────────────────────────────────────────────────
// url()
// ─────────────────────────────────────────────────────────────────────────────

/// Converts a string to a URL. Returns an error if the string is not a valid
/// absolute URI or absolute path.
///
/// # Safety
/// `str_ptr` must be a valid, non-null pointer to a `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_url_parse(str_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if str_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_url_parse");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*str_ptr };
    let s = match val {
        CelValue::String(s) => s.as_str(),
        other => {
            error!(log, "expected String"; "function" => "cel_k8s_url_parse", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    match is_valid_request_uri(s) {
        Ok((u, original)) => Box::into_raw(Box::new(CelValue::Url(u, original))),
        Err(msg) => {
            error!(log, "invalid URL"; "function" => "cel_k8s_url_parse", "error" => &msg);
            create_error_value(&msg)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// isURL()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the string is a valid absolute URI or absolute path.
///
/// # Safety
/// `str_ptr` must be a valid, non-null pointer to a `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_is_url(str_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if str_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_is_url");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*str_ptr };
    let s = match val {
        CelValue::String(s) => s.as_str(),
        other => {
            error!(log, "expected String"; "function" => "cel_k8s_is_url", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let ok = is_valid_request_uri(s).is_ok();
    Box::into_raw(Box::new(CelValue::Bool(ok)))
}

// ─────────────────────────────────────────────────────────────────────────────
// getScheme()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the scheme of the URL, or an empty string if absent.
///
/// # Safety
/// `url_ptr` must be a valid, non-null pointer to a `CelValue::Url`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_url_get_scheme(url_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if url_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_url_get_scheme");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*url_ptr };
    match val {
        CelValue::Url(u, original) => {
            let scheme = if is_path_only(original) {
                String::new()
            } else {
                u.scheme().to_string()
            };
            Box::into_raw(Box::new(CelValue::String(scheme)))
        }
        other => {
            error!(log, "expected URL"; "function" => "cel_k8s_url_get_scheme", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// getHost()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the host:port of the URL. IPv6 addresses include brackets.
/// Returns an empty string if absent.
///
/// # Safety
/// `url_ptr` must be a valid, non-null pointer to a `CelValue::Url`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_url_get_host(url_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if url_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_url_get_host");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*url_ptr };
    match val {
        CelValue::Url(u, original) => {
            let host = if is_path_only(original) {
                String::new()
            } else {
                // u.host_str() returns host without port; we need host+port.
                // u.authority() returns "user:pass@host:port" — we want just host:port.
                match (u.host_str(), u.port()) {
                    (None, _) => String::new(),
                    (Some(h), None) => h.to_string(),
                    (Some(h), Some(p)) => format!("{}:{}", h, p),
                }
            };
            Box::into_raw(Box::new(CelValue::String(host)))
        }
        other => {
            error!(log, "expected URL"; "function" => "cel_k8s_url_get_host", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// getHostname()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the hostname of the URL (port stripped, IPv6 brackets stripped).
/// Returns an empty string if absent.
///
/// # Safety
/// `url_ptr` must be a valid, non-null pointer to a `CelValue::Url`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_url_get_hostname(url_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if url_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_url_get_hostname");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*url_ptr };
    match val {
        CelValue::Url(u, original) => {
            let hostname = if is_path_only(original) {
                String::new()
            } else {
                // Use the Host enum so IPv6 addresses are returned without brackets.
                // host_str() includes brackets for IPv6 (e.g. "[::1]"), but the
                // k8s spec requires getHostname() to strip them (→ "::1").
                match u.host() {
                    None => String::new(),
                    Some(Host::Domain(s)) => s.to_string(),
                    Some(Host::Ipv4(addr)) => addr.to_string(),
                    Some(Host::Ipv6(addr)) => addr.to_string(), // no brackets
                }
            };
            Box::into_raw(Box::new(CelValue::String(hostname)))
        }
        other => {
            error!(log, "expected URL"; "function" => "cel_k8s_url_get_hostname", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// getPort()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the port of the URL as a string, or an empty string if absent.
///
/// # Safety
/// `url_ptr` must be a valid, non-null pointer to a `CelValue::Url`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_url_get_port(url_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if url_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_url_get_port");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*url_ptr };
    match val {
        CelValue::Url(u, original) => {
            let port = if is_path_only(original) {
                String::new()
            } else {
                u.port().map(|p| p.to_string()).unwrap_or_default()
            };
            Box::into_raw(Box::new(CelValue::String(port)))
        }
        other => {
            error!(log, "expected URL"; "function" => "cel_k8s_url_get_port", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// getEscapedPath()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the percent-encoded path of the URL, or an empty string if absent.
///
/// # Safety
/// `url_ptr` must be a valid, non-null pointer to a `CelValue::Url`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_url_get_escaped_path(url_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if url_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_url_get_escaped_path");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*url_ptr };
    match val {
        CelValue::Url(u, original) => {
            // url::Url normalises URLs without an explicit path to path="/".
            // e.g. "https://example.com" → path "/". The k8s spec (matching Go's
            // url.EscapedPath) returns "" in that case, not "/".
            //
            // For path-only URLs the original string IS the path.
            // For absolute URIs we check the original for an explicit path component.
            let path = if is_path_only(original) {
                original.to_string()
            } else {
                let raw = u.path();
                // If the normalised path is "/" and the original string didn't
                // contain an explicit path (no '/' after the authority), return "".
                if raw == "/"
                    && !original[original.find("://").map_or(0, |i| i + 3)..].contains('/')
                {
                    String::new()
                } else {
                    raw.to_string()
                }
            };
            Box::into_raw(Box::new(CelValue::String(path)))
        }
        other => {
            error!(log, "expected URL"; "function" => "cel_k8s_url_get_escaped_path", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// getQuery()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the query parameters as `map<string, list<string>>`.
/// Repeated keys produce multiple values in the list.
/// Returns an empty map if no query string is present.
///
/// # Safety
/// `url_ptr` must be a valid, non-null pointer to a `CelValue::Url`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_url_get_query(url_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if url_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_url_get_query");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*url_ptr };
    match val {
        CelValue::Url(u, _original) => {
            // Collect query params into a map<string, list<string>>.
            let mut map: HashMap<CelMapKey, CelValue> = HashMap::new();

            for (key, value) in u.query_pairs() {
                let k = CelMapKey::String(key.into_owned());
                let v = CelValue::String(value.into_owned());
                match map.get_mut(&k) {
                    Some(CelValue::Array(arr)) => arr.push(v),
                    Some(_) => unreachable!(),
                    None => {
                        map.insert(k, CelValue::Array(vec![v]));
                    }
                }
            }

            Box::into_raw(Box::new(CelValue::Object(map)))
        }
        other => {
            error!(log, "expected URL"; "function" => "cel_k8s_url_get_query", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{make_str, make_val, read_val};
    use super::*;
    use rstest::rstest;

    unsafe fn make_url(s: &str) -> *mut CelValue {
        let str_ptr = unsafe { make_str(s) };
        unsafe { cel_k8s_url_parse(str_ptr) }
    }

    // ── url() / isURL() ──────────────────────────────────────────────────────

    #[rstest]
    #[case::full_url("https://user:pass@example.com:80/path?query=val#fragment")]
    #[case::simple_https("https://example.com/")]
    #[case::absolute_path("/absolute-path")]
    #[case::path_with_query("/path?k=v")]
    #[case::ipv6("https://[::1]:80/path")]
    fn test_url_parse_valid(#[case] input: &str) {
        let str_ptr = unsafe { make_str(input) };
        let result = unsafe { read_val(cel_k8s_url_parse(str_ptr)) };
        assert!(
            matches!(result, CelValue::Url(_, _)),
            "expected Url for {:?}, got {:?}",
            input,
            result
        );
        unsafe { drop(Box::from_raw(str_ptr)) };
    }

    #[rstest]
    #[case::relative_path("../relative-path")]
    #[case::invalid_scheme("https://a:b:c/")]
    fn test_url_parse_invalid(#[case] input: &str) {
        let str_ptr = unsafe { make_str(input) };
        let result = unsafe { read_val(cel_k8s_url_parse(str_ptr)) };
        assert!(
            matches!(result, CelValue::Error(_)),
            "expected Error for {:?}, got {:?}",
            input,
            result
        );
        unsafe { drop(Box::from_raw(str_ptr)) };
    }

    #[rstest]
    #[case::full_url("https://user:pass@example.com:80/path?query=val#fragment", true)]
    #[case::absolute_path("/absolute-path", true)]
    #[case::relative_path("../relative-path", false)]
    #[case::invalid_scheme("https://a:b:c/", false)]
    fn test_is_url(#[case] input: &str, #[case] expected: bool) {
        let str_ptr = unsafe { make_str(input) };
        let result = unsafe { read_val(cel_k8s_is_url(str_ptr)) };
        assert_eq!(result, CelValue::Bool(expected), "isURL({:?})", input);
        unsafe { drop(Box::from_raw(str_ptr)) };
    }

    // ── getScheme() ──────────────────────────────────────────────────────────

    #[rstest]
    #[case::https("https://example.com/", "https")]
    #[case::http("http://example.com/", "http")]
    #[case::absolute_path("/path", "")]
    fn test_get_scheme(#[case] url_str: &str, #[case] expected: &str) {
        let url_ptr = unsafe { make_url(url_str) };
        let result = unsafe { read_val(cel_k8s_url_get_scheme(url_ptr)) };
        assert_eq!(
            result,
            CelValue::String(expected.to_string()),
            "getScheme({:?})",
            url_str
        );
    }

    // ── getHost() ────────────────────────────────────────────────────────────

    #[rstest]
    #[case::with_port("https://example.com:80/", "example.com:80")]
    #[case::without_port("https://example.com/", "example.com")]
    #[case::ipv6_with_port("https://[::1]:80/", "[::1]:80")]
    #[case::ipv6_without_port("https://[::1]/", "[::1]")]
    #[case::absolute_path("/path", "")]
    fn test_get_host(#[case] url_str: &str, #[case] expected: &str) {
        let url_ptr = unsafe { make_url(url_str) };
        let result = unsafe { read_val(cel_k8s_url_get_host(url_ptr)) };
        assert_eq!(
            result,
            CelValue::String(expected.to_string()),
            "getHost({:?})",
            url_str
        );
    }

    // ── getHostname() ────────────────────────────────────────────────────────

    #[rstest]
    #[case::with_port("https://example.com:80/", "example.com")]
    #[case::without_port("https://example.com/", "example.com")]
    #[case::ipv4_with_port("https://127.0.0.1:80/", "127.0.0.1")]
    #[case::ipv6_with_port("https://[::1]:80/", "::1")]
    #[case::absolute_path("/path", "")]
    fn test_get_hostname(#[case] url_str: &str, #[case] expected: &str) {
        let url_ptr = unsafe { make_url(url_str) };
        let result = unsafe { read_val(cel_k8s_url_get_hostname(url_ptr)) };
        assert_eq!(
            result,
            CelValue::String(expected.to_string()),
            "getHostname({:?})",
            url_str
        );
    }

    // ── getPort() ────────────────────────────────────────────────────────────

    #[rstest]
    #[case::with_port("https://example.com:80/", "80")]
    #[case::without_port("https://example.com/", "")]
    #[case::absolute_path("/path", "")]
    fn test_get_port(#[case] url_str: &str, #[case] expected: &str) {
        let url_ptr = unsafe { make_url(url_str) };
        let result = unsafe { read_val(cel_k8s_url_get_port(url_ptr)) };
        assert_eq!(
            result,
            CelValue::String(expected.to_string()),
            "getPort({:?})",
            url_str
        );
    }

    // ── getEscapedPath() ─────────────────────────────────────────────────────

    #[rstest]
    #[case::simple_path("https://example.com/path", "/path")]
    #[case::no_path("https://example.com", "")]
    #[case::encoded_path("https://example.com/path%20with%20spaces/", "/path%20with%20spaces/")]
    #[case::absolute_path("/path", "/path")]
    fn test_get_escaped_path(#[case] url_str: &str, #[case] expected: &str) {
        let url_ptr = unsafe { make_url(url_str) };
        let result = unsafe { read_val(cel_k8s_url_get_escaped_path(url_ptr)) };
        assert_eq!(
            result,
            CelValue::String(expected.to_string()),
            "getEscapedPath({:?})",
            url_str
        );
    }

    // ── getQuery() ───────────────────────────────────────────────────────────

    #[test]
    fn test_get_query_empty() {
        let url_ptr = unsafe { make_url("https://example.com/path") };
        let result = unsafe { read_val(cel_k8s_url_get_query(url_ptr)) };
        assert_eq!(
            result,
            CelValue::Object(HashMap::new()),
            "getQuery() on URL with no query"
        );
    }

    #[test]
    fn test_get_query_empty_string() {
        let url_ptr = unsafe { make_url("https://example.com/path?") };
        let result = unsafe { read_val(cel_k8s_url_get_query(url_ptr)) };
        assert_eq!(
            result,
            CelValue::Object(HashMap::new()),
            "getQuery() on URL with empty query string"
        );
    }

    #[test]
    fn test_get_query_single_key() {
        let url_ptr = unsafe { make_url("https://example.com/path?k1=a") };
        let result = unsafe { read_val(cel_k8s_url_get_query(url_ptr)) };
        let mut expected = HashMap::new();
        expected.insert(
            CelMapKey::String("k1".to_string()),
            CelValue::Array(vec![CelValue::String("a".to_string())]),
        );
        assert_eq!(result, CelValue::Object(expected));
    }

    #[test]
    fn test_get_query_repeated_keys() {
        // k2 appears twice → list with two values, preserving order
        let url_ptr = unsafe { make_url("https://example.com/path?k1=a&k2=b&k2=c") };
        let result = unsafe { read_val(cel_k8s_url_get_query(url_ptr)) };
        // Unpack to verify k1 and k2 independently (HashMap order is unspecified)
        if let CelValue::Object(map) = result {
            let k1 = map.get(&CelMapKey::String("k1".to_string())).unwrap();
            assert_eq!(
                k1,
                &CelValue::Array(vec![CelValue::String("a".to_string())])
            );
            let k2 = map.get(&CelMapKey::String("k2".to_string())).unwrap();
            assert_eq!(
                k2,
                &CelValue::Array(vec![
                    CelValue::String("b".to_string()),
                    CelValue::String("c".to_string()),
                ])
            );
        } else {
            panic!("expected Object, got {:?}", result);
        }
    }

    #[test]
    fn test_get_query_percent_encoded_keys_and_values() {
        // The spec says keys and values are returned unescaped
        let url_ptr = unsafe {
            make_url("https://example.com/path?key%20with%20spaces=value%20with%20spaces")
        };
        let result = unsafe { read_val(cel_k8s_url_get_query(url_ptr)) };
        let mut expected = HashMap::new();
        expected.insert(
            CelMapKey::String("key with spaces".to_string()),
            CelValue::Array(vec![CelValue::String("value with spaces".to_string())]),
        );
        assert_eq!(result, CelValue::Object(expected));
    }

    // ── wrong type returns error ─────────────────────────────────────────────

    #[test]
    fn test_get_scheme_wrong_type_returns_error() {
        let val_ptr = unsafe { make_val(CelValue::Int(42)) };
        let result = unsafe { read_val(cel_k8s_url_get_scheme(val_ptr)) };
        assert!(matches!(result, CelValue::Error(_)));
        unsafe { drop(Box::from_raw(val_ptr)) };
    }
}
