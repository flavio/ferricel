use crate::common::*;
use rstest::rstest;

// ── isCIDR() ──────────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"isCIDR("192.168.0.0/24")"#, true)]
#[case(r#"isCIDR("10.0.0.0/8")"#, true)]
#[case(r#"isCIDR("2001:db8::/32")"#, true)]
#[case(r#"isCIDR("0.0.0.0/0")"#, true)]
#[case(r#"isCIDR("192.168.0.0/33")"#, false)] // prefix too large
#[case(r#"isCIDR("192.168.0.0/")"#, false)] // empty prefix
#[case(r#"isCIDR("192.168.0.0")"#, false)] // no prefix
#[case(r#"isCIDR("not-a-cidr")"#, false)]
#[case(r#"isCIDR("fe80::1%en0/24")"#, false)] // zone id
fn test_is_cidr(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── cidr() parse ──────────────────────────────────────────────────────────────

#[test]
fn test_cidr_parse_valid_ipv4() {
    let result = compile_and_execute_bool(r#"type(cidr("192.168.0.0/24")) == net.CIDR"#)
        .expect("Failed to compile and execute");
    assert!(result, "type(cidr()) should equal net.CIDR");
}

#[test]
fn test_cidr_parse_valid_ipv6() {
    let result = compile_and_execute_bool(r#"type(cidr("2001:db8::/32")) == net.CIDR"#)
        .expect("Failed to compile and execute");
    assert!(result, "type(cidr()) should equal net.CIDR");
}

#[test]
fn test_cidr_parse_invalid_returns_error() {
    let result = compile_and_execute(r#"cidr("not-a-cidr")"#);
    assert!(
        result.is_err(),
        "cidr() with invalid input should return error"
    );
}

// ── cidr().ip() ───────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"cidr("192.168.0.0/24").ip() == ip("192.168.0.0")"#, true)]
#[case(r#"cidr("2001:db8::/32").ip() == ip("2001:db8::")"#, true)]
fn test_cidr_ip(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── cidr().masked() ───────────────────────────────────────────────────────────

#[rstest]
#[case(r#"cidr("192.168.0.1/24").masked() == cidr("192.168.0.0/24")"#, true)]
#[case(r#"cidr("192.168.0.0/24").masked() == cidr("192.168.0.0/24")"#, true)]
fn test_cidr_masked(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── cidr().prefixLength() ─────────────────────────────────────────────────────

#[rstest]
#[case(r#"cidr("192.168.0.0/24").prefixLength()"#, 24)]
#[case(r#"cidr("10.0.0.0/8").prefixLength()"#, 8)]
#[case(r#"cidr("2001:db8::/32").prefixLength()"#, 32)]
fn test_cidr_prefix_length(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── cidr().containsIP() ──────────────────────────────────────────────────────

#[rstest]
#[case(r#"cidr("192.168.0.0/24").containsIP(ip("192.168.0.1"))"#, true)]
#[case(r#"cidr("192.168.0.0/24").containsIP(ip("192.168.1.1"))"#, false)]
#[case(r#"cidr("192.168.0.0/24").containsIP("192.168.0.1")"#, true)]
#[case(r#"cidr("192.168.0.0/24").containsIP("192.168.1.1")"#, false)]
#[case(r#"cidr("2001:db8::/32").containsIP(ip("2001:db8::1"))"#, true)]
#[case(r#"cidr("2001:db8::/32").containsIP(ip("192.168.1.1"))"#, false)] // cross-family
#[case(r#"cidr("192.168.1.1/32").containsIP(ip("2001:db8::1"))"#, false)] // cross-family
fn test_cidr_contains_ip(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── cidr().containsCIDR() ────────────────────────────────────────────────────

#[rstest]
#[case(r#"cidr("192.168.0.0/24").containsCIDR(cidr("192.168.0.0/25"))"#, true)]
#[case(r#"cidr("192.168.0.0/24").containsCIDR(cidr("192.168.0.1/32"))"#, true)]
#[case(
    r#"cidr("192.168.0.0/24").containsCIDR(cidr("192.168.0.0/23"))"#,
    false
)] // superset
#[case(r#"cidr("10.0.0.0/8").containsCIDR(cidr("10.0.0.0/8"))"#, true)] // equal
#[case(r#"cidr("192.168.0.0/24").containsCIDR("192.168.0.0/25")"#, true)]
#[case(r#"cidr("10.0.0.0/8").containsCIDR("10.0.0.0/8")"#, true)]
#[case(r#"cidr("2001:db8::/32").containsCIDR(cidr("2001:db8::/33"))"#, true)]
fn test_cidr_contains_cidr(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── cidr() equality ──────────────────────────────────────────────────────────

#[rstest]
#[case(r#"cidr("127.0.0.1/24") == cidr("127.0.0.1/24")"#, true)]
#[case(r#"cidr("192.0.0.1/32") == cidr("10.0.0.1/8")"#, false)]
#[case(r#"cidr("2001:db8::/32") == cidr("10.0.0.1/32")"#, false)]
fn test_cidr_equality(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute_bool(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── cidr() string conversion ──────────────────────────────────────────────────

#[test]
fn test_cidr_to_string_ipv4() {
    let result = compile_and_execute_string(r#"string(cidr("192.168.0.0/24"))"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, "192.168.0.0/24");
}

#[test]
fn test_cidr_to_string_ipv6() {
    let result = compile_and_execute_string(r#"string(cidr("2001:db8::/32"))"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, "2001:db8::/32");
}
