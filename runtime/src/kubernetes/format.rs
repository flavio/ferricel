//! Kubernetes CEL Format library runtime functions.
//!
//! Implements the Kubernetes format library described in:
//!   <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-format-library>
//!
//! Functions:
//!   - `format.named(string)`                  → ?Format (optional)
//!   - `format.<formatName>()`                 → Format  (13 zero-arg constructors)
//!   - `<Format>.validate(string)`             → ?`list<string>`
//!
//! The 13 named formats:
//!   dns1123Label, dns1123Subdomain, dns1035Label, qualifiedName,
//!   dns1123LabelPrefix, dns1123SubdomainPrefix, dns1035LabelPrefix,
//!   labelValue, uri, uuid, byte, date, datetime
//!
//! Reference port of:
//!   k8s.io/apiserver/pkg/cel/library/format.go
//!   k8s.io/apimachinery/pkg/util/validation/validation.go

use crate::error::{create_error_value, read_ptr};
use crate::types::CelValue;
use base64::Engine;
use chrono::NaiveDate;
use regex_lite::Regex;
use slog::error;
use std::sync::OnceLock;
use url::Url;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Regex constants (compiled once)
// ─────────────────────────────────────────────────────────────────────────────

fn dns1123_label_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?$").unwrap())
}

fn dns1123_subdomain_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let label = r"[a-z0-9]([-a-z0-9]*[a-z0-9])?";
        Regex::new(&format!(r"^{label}(\.{label})*$")).unwrap()
    })
}

fn dns1035_label_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z]([-a-z0-9]*[a-z0-9])?$").unwrap())
}

/// Label value pattern: empty or `([A-Za-z0-9][-A-Za-z0-9_.]*)?[A-Za-z0-9]`
fn label_value_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(([A-Za-z0-9][-A-Za-z0-9_.]*)?[A-Za-z0-9])?$").unwrap())
}

// ─────────────────────────────────────────────────────────────────────────────
// Validation helpers (return `Vec<String>` of errors, empty = valid)
// ─────────────────────────────────────────────────────────────────────────────

const DNS1123_LABEL_MAX: usize = 63;
const DNS1123_SUBDOMAIN_MAX: usize = 253;
const DNS1035_LABEL_MAX: usize = 63;
const LABEL_VALUE_MAX: usize = 63;

fn max_len_error(n: usize) -> String {
    format!("must be no more than {} characters", n)
}

fn validate_dns1123_label(value: &str) -> Vec<String> {
    let mut errs = Vec::new();
    if value.len() > DNS1123_LABEL_MAX {
        errs.push(max_len_error(DNS1123_LABEL_MAX));
    }
    if !dns1123_label_re().is_match(value) {
        if dns1123_subdomain_re().is_match(value) {
            errs.push("must not contain dots".to_string());
        } else {
            errs.push(
                "a lowercase RFC 1123 label must consist of lower case alphanumeric characters or '-', and must start and end with an alphanumeric character \
                 (e.g. 'my-name',  or '123-abc', regex used for validation is '[a-z0-9]([-a-z0-9]*[a-z0-9])?')"
                    .to_string(),
            );
        }
    }
    errs
}

fn validate_dns1123_subdomain(value: &str) -> Vec<String> {
    let mut errs = Vec::new();
    if value.len() > DNS1123_SUBDOMAIN_MAX {
        errs.push(max_len_error(DNS1123_SUBDOMAIN_MAX));
    }
    if !dns1123_subdomain_re().is_match(value) {
        errs.push(
            "a lowercase RFC 1123 subdomain must consist of lower case alphanumeric characters, '-' or '.', and must start and end with an alphanumeric character \
             (e.g. 'example.com', regex used for validation is '[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*')"
                .to_string(),
        );
    }
    errs
}

fn validate_dns1035_label(value: &str) -> Vec<String> {
    let mut errs = Vec::new();
    if value.len() > DNS1035_LABEL_MAX {
        errs.push(max_len_error(DNS1035_LABEL_MAX));
    }
    if !dns1035_label_re().is_match(value) {
        errs.push(
            "a DNS-1035 label must consist of lower case alphanumeric characters or '-', start with an alphabetic character, and end with an alphanumeric character \
             (e.g. 'my-name',  or 'abc-123', regex used for validation is '[a-z]([-a-z0-9]*[a-z0-9])?')"
                .to_string(),
        );
    }
    errs
}

/// `qualifiedName` validation (matches k8s `IsQualifiedName` / `IsLabelKey`).
///
/// A qualified name is either:
/// - A simple name: `[A-Za-z0-9]([-A-Za-z0-9_.]*[A-Za-z0-9])?` (≤ 63 chars)
/// - A prefixed name: `<dns1123-subdomain>/<simple-name>`
fn validate_qualified_name(value: &str) -> Vec<String> {
    let mut errs = Vec::new();
    if let Some(slash) = value.find('/') {
        let prefix = &value[..slash];
        let name = &value[slash + 1..];
        if prefix.is_empty() {
            errs.push("prefix part must be non-empty".to_string());
        } else {
            errs.extend(validate_dns1123_subdomain(prefix));
        }
        errs.extend(validate_simple_qualified_name(name));
    } else {
        errs.extend(validate_simple_qualified_name(value));
    }
    errs
}

fn validate_simple_qualified_name(value: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^([A-Za-z0-9][-A-Za-z0-9_.]*)?[A-Za-z0-9]$").unwrap());
    let mut errs = Vec::new();
    if value.is_empty() {
        errs.push("name part must be non-empty".to_string());
        return errs;
    }
    if value.len() > 63 {
        errs.push(max_len_error(63));
    }
    if !re.is_match(value) {
        errs.push(
            "name part must consist of alphanumeric characters, '-', '_' or '.', and must start and end with an alphanumeric character \
             (e.g. 'MyName',  or 'my.name',  or '123-abc', regex used for validation is '([A-Za-z0-9][-A-Za-z0-9_.]*)?[A-Za-z0-9]')"
                .to_string(),
        );
    }
    errs
}

fn validate_label_value(value: &str) -> Vec<String> {
    let mut errs = Vec::new();
    if value.len() > LABEL_VALUE_MAX {
        errs.push(max_len_error(LABEL_VALUE_MAX));
    }
    // Empty string is valid for label values
    if !value.is_empty() && !label_value_re().is_match(value) {
        errs.push(
            "a valid label must be an empty string or consist of alphanumeric characters, '-', '_' or '.', \
             and must start and end with an alphanumeric character \
             (e.g. 'MyValue',  or 'my_value',  or '12345', regex used for validation is '(([A-Za-z0-9][-A-Za-z0-9_.]*)?[A-Za-z0-9])?')"
                .to_string(),
        );
    }
    errs
}

fn validate_uri(value: &str) -> Vec<String> {
    // Mirrors Go's url.ParseRequestURI: accepts absolute URIs and absolute paths.
    if value.starts_with('/') {
        // Absolute path — valid
        return Vec::new();
    }
    match Url::parse(value) {
        Ok(u) if !u.cannot_be_a_base() => Vec::new(),
        Ok(_) => vec![format!("invalid URI: relative URL not allowed: {}", value)],
        Err(e) => vec![e.to_string()],
    }
}

fn validate_uuid(value: &str) -> Vec<String> {
    if Uuid::parse_str(value).is_ok() {
        Vec::new()
    } else {
        vec!["does not match the UUID format".to_string()]
    }
}

fn validate_byte(value: &str) -> Vec<String> {
    // Standard base64 or base64url, with or without padding
    if base64::engine::general_purpose::STANDARD
        .decode(value)
        .is_ok()
        || base64::engine::general_purpose::URL_SAFE
            .decode(value)
            .is_ok()
        || base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(value)
            .is_ok()
        || base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(value)
            .is_ok()
    {
        Vec::new()
    } else {
        vec!["invalid base64".to_string()]
    }
}

fn validate_date(value: &str) -> Vec<String> {
    // RFC3339 full-date: YYYY-MM-DD
    if NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok() {
        Vec::new()
    } else {
        vec!["invalid date".to_string()]
    }
}

fn validate_datetime(value: &str) -> Vec<String> {
    // RFC3339 date-time
    if value.parse::<chrono::DateTime<chrono::Utc>>().is_ok()
        || chrono::DateTime::parse_from_rfc3339(value).is_ok()
    {
        Vec::new()
    } else {
        vec!["invalid datetime".to_string()]
    }
}

/// For prefix variants: mask a trailing dash before validation (so a trailing dash is allowed
/// in prefix names — the generated suffix will make it valid).
fn mask_trailing_dash(s: &str) -> &str {
    if s.len() > 1 && s.ends_with('-') {
        &s[..s.len() - 1]
    } else {
        s
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Dispatch by format name
// ─────────────────────────────────────────────────────────────────────────────

fn validate_by_name(format_name: &str, value: &str) -> Option<Vec<String>> {
    let errs = match format_name {
        "dns1123Label" => validate_dns1123_label(value),
        "dns1123Subdomain" => validate_dns1123_subdomain(value),
        "dns1035Label" => validate_dns1035_label(value),
        "qualifiedName" => validate_qualified_name(value),
        "dns1123LabelPrefix" => validate_dns1123_label(mask_trailing_dash(value)),
        "dns1123SubdomainPrefix" => validate_dns1123_subdomain(mask_trailing_dash(value)),
        "dns1035LabelPrefix" => validate_dns1035_label(mask_trailing_dash(value)),
        "labelValue" => validate_label_value(value),
        "uri" => validate_uri(value),
        "uuid" => validate_uuid(value),
        "byte" => validate_byte(value),
        "date" => validate_date(value),
        "datetime" => validate_datetime(value),
        _ => return None,
    };
    Some(errs)
}

// ─────────────────────────────────────────────────────────────────────────────
// WASM-callable runtime functions
// ─────────────────────────────────────────────────────────────────────────────

/// `format.named(name: string)` → `?Format`
///
/// Returns `Optional(Some(Format(name)))` if `name` is a known format,
/// `Optional(None)` otherwise.
///
/// # Safety
/// `name_ptr` must be a valid, non-null pointer to a `CelValue::String`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_format_named(name_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();
    if name_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_format_named");
        return create_error_value("no such overload");
    }
    let name_val = unsafe { read_ptr(name_ptr) };
    let name = match name_val {
        CelValue::String(s) => s,
        other => {
            error!(log, "expected string"; "function" => "cel_k8s_format_named", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };
    // Check if format name is known
    let known = matches!(
        name.as_str(),
        "dns1123Label"
            | "dns1123Subdomain"
            | "dns1035Label"
            | "qualifiedName"
            | "dns1123LabelPrefix"
            | "dns1123SubdomainPrefix"
            | "dns1035LabelPrefix"
            | "labelValue"
            | "uri"
            | "uuid"
            | "byte"
            | "date"
            | "datetime"
    );
    if known {
        Box::into_raw(Box::new(CelValue::Optional(Some(Box::new(
            CelValue::Format(name),
        )))))
    } else {
        Box::into_raw(Box::new(CelValue::Optional(None)))
    }
}

// Zero-argument format constructors — each returns `CelValue::Format("<name>")`

macro_rules! format_constructor {
    ($fn_name:ident, $format_name:literal) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $fn_name() -> *mut CelValue {
            Box::into_raw(Box::new(CelValue::Format($format_name.to_string())))
        }
    };
}

format_constructor!(cel_k8s_format_dns1123_label, "dns1123Label");
format_constructor!(cel_k8s_format_dns1123_subdomain, "dns1123Subdomain");
format_constructor!(cel_k8s_format_dns1035_label, "dns1035Label");
format_constructor!(cel_k8s_format_qualified_name, "qualifiedName");
format_constructor!(cel_k8s_format_dns1123_label_prefix, "dns1123LabelPrefix");
format_constructor!(
    cel_k8s_format_dns1123_subdomain_prefix,
    "dns1123SubdomainPrefix"
);
format_constructor!(cel_k8s_format_dns1035_label_prefix, "dns1035LabelPrefix");
format_constructor!(cel_k8s_format_label_value, "labelValue");
format_constructor!(cel_k8s_format_uri, "uri");
format_constructor!(cel_k8s_format_uuid, "uuid");
format_constructor!(cel_k8s_format_byte, "byte");
format_constructor!(cel_k8s_format_date, "date");
format_constructor!(cel_k8s_format_datetime, "datetime");

/// `<Format>.validate(str: string)` → `?list<string>`
///
/// Returns `Optional(None)` if the string is valid for the format.
/// Returns `Optional(Some([errors...]))` if it is invalid.
///
/// # Safety
/// Both pointers must be valid, non-null.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_format_validate(
    format_ptr: *mut CelValue,
    value_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();
    if format_ptr.is_null() || value_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_format_validate");
        return create_error_value("no such overload");
    }
    let format_val = unsafe { read_ptr(format_ptr) };
    let format_name = match format_val {
        CelValue::Format(s) => s,
        other => {
            error!(log, "expected Format"; "function" => "cel_k8s_format_validate", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };
    let value_val = unsafe { read_ptr(value_ptr) };
    let value_str = match value_val {
        CelValue::String(s) => s,
        other => {
            error!(log, "expected String"; "function" => "cel_k8s_format_validate", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let errs = match validate_by_name(&format_name, &value_str) {
        Some(e) => e,
        None => {
            error!(log, "unknown format name"; "function" => "cel_k8s_format_validate", "name" => &format_name);
            return create_error_value("unknown format");
        }
    };

    if errs.is_empty() {
        // Valid — return optional.none
        Box::into_raw(Box::new(CelValue::Optional(None)))
    } else {
        // Invalid — return optional.some([errors])
        let error_list = CelValue::Array(errs.into_iter().map(CelValue::String).collect());
        Box::into_raw(Box::new(CelValue::Optional(Some(Box::new(error_list)))))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kubernetes::test_helpers::*;

    unsafe fn read_format(ptr: *mut CelValue) -> String {
        match read_val(ptr) {
            CelValue::Format(s) => s,
            other => panic!("expected Format, got {:?}", other),
        }
    }

    unsafe fn read_optional_list(ptr: *mut CelValue) -> Option<Vec<String>> {
        match read_val(ptr) {
            CelValue::Optional(None) => None,
            CelValue::Optional(Some(inner)) => match *inner {
                CelValue::Array(items) => Some(
                    items
                        .into_iter()
                        .map(|v| match v {
                            CelValue::String(s) => s,
                            other => panic!("expected string in list, got {:?}", other),
                        })
                        .collect(),
                ),
                other => panic!("expected Array inside Optional, got {:?}", other),
            },
            other => panic!("expected Optional, got {:?}", other),
        }
    }

    // ── format constructors ──

    #[test]
    fn test_format_constructors() {
        unsafe {
            assert_eq!(read_format(cel_k8s_format_dns1123_label()), "dns1123Label");
            assert_eq!(
                read_format(cel_k8s_format_dns1123_subdomain()),
                "dns1123Subdomain"
            );
            assert_eq!(read_format(cel_k8s_format_dns1035_label()), "dns1035Label");
            assert_eq!(
                read_format(cel_k8s_format_qualified_name()),
                "qualifiedName"
            );
            assert_eq!(read_format(cel_k8s_format_uri()), "uri");
            assert_eq!(read_format(cel_k8s_format_uuid()), "uuid");
            assert_eq!(read_format(cel_k8s_format_byte()), "byte");
            assert_eq!(read_format(cel_k8s_format_date()), "date");
            assert_eq!(read_format(cel_k8s_format_datetime()), "datetime");
        }
    }

    // ── format.named ──

    #[test]
    fn test_format_named_known() {
        let name_ptr = make_str("dns1123Label");
        let result = unsafe { cel_k8s_format_named(name_ptr) };
        match read_val(result) {
            CelValue::Optional(Some(inner)) => {
                assert_eq!(*inner, CelValue::Format("dns1123Label".to_string()))
            }
            other => panic!("expected Optional(Some(Format)), got {:?}", other),
        }
    }

    #[test]
    fn test_format_named_unknown() {
        let name_ptr = make_str("bogus");
        let result = unsafe { cel_k8s_format_named(name_ptr) };
        assert_eq!(read_val(result), CelValue::Optional(None));
    }

    // ── dns1123Label ──

    #[test]
    fn test_validate_dns1123_label_valid() {
        let fmt = cel_k8s_format_dns1123_label();
        let val = make_str("my-label");
        assert_eq!(
            unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) },
            None
        );
    }

    #[test]
    fn test_validate_dns1123_label_invalid_dots() {
        let fmt = cel_k8s_format_dns1123_label();
        let val = make_str("my.label");
        let errs = unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) }.unwrap();
        assert!(errs.iter().any(|e| e.contains("must not contain dots")));
    }

    #[test]
    fn test_validate_dns1123_label_too_long() {
        let fmt = cel_k8s_format_dns1123_label();
        let long = "a".repeat(64);
        let val = make_str(&long);
        let errs = unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) }.unwrap();
        assert!(errs.iter().any(|e| e.contains("63")));
    }

    // ── dns1123Subdomain ──

    #[test]
    fn test_validate_dns1123_subdomain_valid() {
        let fmt = cel_k8s_format_dns1123_subdomain();
        let val = make_str("apiextensions.k8s.io");
        assert_eq!(
            unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) },
            None
        );
    }

    #[test]
    fn test_validate_dns1123_subdomain_invalid() {
        let fmt = cel_k8s_format_dns1123_subdomain();
        let val = make_str("NOT_VALID");
        let errs = unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) }.unwrap();
        assert!(!errs.is_empty());
    }

    // ── uuid ──

    #[test]
    fn test_validate_uuid_valid() {
        let fmt = cel_k8s_format_uuid();
        let val = make_str("123e4567-e89b-12d3-a456-426614174000");
        assert_eq!(
            unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) },
            None
        );
    }

    #[test]
    fn test_validate_uuid_invalid() {
        let fmt = cel_k8s_format_uuid();
        let val = make_str("not-a-uuid");
        let errs = unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) }.unwrap();
        assert!(errs.iter().any(|e| e.contains("UUID")));
    }

    // ── byte (base64) ──

    #[test]
    fn test_validate_byte_valid() {
        let fmt = cel_k8s_format_byte();
        let val = make_str("aGVsbG8=");
        assert_eq!(
            unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) },
            None
        );
    }

    #[test]
    fn test_validate_byte_invalid() {
        let fmt = cel_k8s_format_byte();
        let val = make_str("!not!base64!");
        let errs = unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) }.unwrap();
        assert!(errs.iter().any(|e| e.contains("base64")));
    }

    // ── date ──

    #[test]
    fn test_validate_date_valid() {
        let fmt = cel_k8s_format_date();
        let val = make_str("2021-01-01");
        assert_eq!(
            unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) },
            None
        );
    }

    #[test]
    fn test_validate_date_invalid() {
        let fmt = cel_k8s_format_date();
        let val = make_str("2021-13-01");
        let errs = unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) }.unwrap();
        assert!(errs.iter().any(|e| e.contains("invalid date")));
    }

    // ── datetime ──

    #[test]
    fn test_validate_datetime_valid() {
        let fmt = cel_k8s_format_datetime();
        let val = make_str("2021-01-01T00:00:00Z");
        assert_eq!(
            unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) },
            None
        );
    }

    #[test]
    fn test_validate_datetime_invalid() {
        let fmt = cel_k8s_format_datetime();
        let val = make_str("not-a-datetime");
        let errs = unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) }.unwrap();
        assert!(errs.iter().any(|e| e.contains("invalid datetime")));
    }

    // ── uri ──

    #[test]
    fn test_validate_uri_valid() {
        let fmt = cel_k8s_format_uri();
        let val = make_str("http://example.com");
        assert_eq!(
            unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) },
            None
        );
    }

    #[test]
    fn test_validate_uri_invalid() {
        let fmt = cel_k8s_format_uri();
        let val = make_str("not a url");
        let errs = unsafe { read_optional_list(cel_k8s_format_validate(fmt, val)) }.unwrap();
        assert!(!errs.is_empty());
    }
}
