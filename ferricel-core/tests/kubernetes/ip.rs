use crate::common::*;
use rstest::rstest;

// ── isIP() ───────────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"isIP("127.0.0.1")"#, 1)]
#[case(r#"isIP("0.0.0.0")"#, 1)]
#[case(r#"isIP("255.255.255.255")"#, 1)]
#[case(r#"isIP("::1")"#, 1)]
#[case(r#"isIP("::")"#, 1)]
#[case(r#"isIP("2001:db8::abcd")"#, 1)]
#[case(r#"isIP("::ffff:c0a8:1")"#, 1)] // pure-hex IPv4-mapped: allowed
#[case(r#"isIP("not-an-ip")"#, 0)]
#[case(r#"isIP("::ffff:1.2.3.4")"#, 0)] // dotted-quad IPv4-mapped: rejected
#[case(r#"isIP("fe80::1%eth0")"#, 0)]
#[case(r#"isIP("010.0.0.1")"#, 0)]
fn test_is_ip(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── ip.isCanonical() ─────────────────────────────────────────────────────────

#[rstest]
#[case(r#"ip.isCanonical("127.0.0.1")"#, 1)]
#[case(r#"ip.isCanonical("0.0.0.0")"#, 1)]
#[case(r#"ip.isCanonical("255.255.255.255")"#, 1)]
#[case(r#"ip.isCanonical("2001:db8::abcd")"#, 1)]
#[case(r#"ip.isCanonical("2001:DB8::ABCD")"#, 0)]
#[case(r#"ip.isCanonical("2001:db8::0:0:0:abcd")"#, 0)]
fn test_ip_is_canonical(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

#[test]
fn test_ip_is_canonical_invalid_returns_error() {
    // ip.isCanonical on invalid input must propagate an error (not return false).
    let result = compile_and_execute(r#"ip.isCanonical("not-an-ip")"#);
    assert!(
        result.is_err(),
        "Expected error for ip.isCanonical on invalid input, got {:?}",
        result
    );
}

// ── ip().family() ────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"ip("127.0.0.1").family()"#, 4)]
#[case(r#"ip("0.0.0.0").family()"#, 4)]
#[case(r#"ip("255.255.255.255").family()"#, 4)]
#[case(r#"ip("::1").family()"#, 6)]
#[case(r#"ip("::").family()"#, 6)]
#[case(r#"ip("2001:db8::abcd").family()"#, 6)]
fn test_ip_family(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── ip().isUnspecified() ──────────────────────────────────────────────────────

#[rstest]
#[case(r#"ip("0.0.0.0").isUnspecified()"#, 1)]
#[case(r#"ip("::").isUnspecified()"#, 1)]
#[case(r#"ip("127.0.0.1").isUnspecified()"#, 0)]
#[case(r#"ip("::1").isUnspecified()"#, 0)]
fn test_ip_is_unspecified(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── ip().isLoopback() ─────────────────────────────────────────────────────────

#[rstest]
#[case(r#"ip("127.0.0.1").isLoopback()"#, 1)]
#[case(r#"ip("127.1.2.3").isLoopback()"#, 1)]
#[case(r#"ip("::1").isLoopback()"#, 1)]
#[case(r#"ip("192.168.0.1").isLoopback()"#, 0)]
#[case(r#"ip("2001:db8::abcd").isLoopback()"#, 0)]
fn test_ip_is_loopback(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── ip().isLinkLocalMulticast() ───────────────────────────────────────────────

#[rstest]
#[case(r#"ip("224.0.0.1").isLinkLocalMulticast()"#, 1)]
#[case(r#"ip("224.0.0.255").isLinkLocalMulticast()"#, 1)]
#[case(r#"ip("224.0.1.1").isLinkLocalMulticast()"#, 0)]
#[case(r#"ip("ff02::1").isLinkLocalMulticast()"#, 1)]
#[case(r#"ip("192.168.0.1").isLinkLocalMulticast()"#, 0)]
fn test_ip_is_link_local_multicast(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── ip().isLinkLocalUnicast() ─────────────────────────────────────────────────

#[rstest]
#[case(r#"ip("169.254.169.254").isLinkLocalUnicast()"#, 1)]
#[case(r#"ip("169.254.0.1").isLinkLocalUnicast()"#, 1)]
#[case(r#"ip("192.168.0.1").isLinkLocalUnicast()"#, 0)]
#[case(r#"ip("fe80::1").isLinkLocalUnicast()"#, 1)]
#[case(r#"ip("fd80::1").isLinkLocalUnicast()"#, 0)]
fn test_ip_is_link_local_unicast(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── ip().isGlobalUnicast() ────────────────────────────────────────────────────

#[rstest]
#[case(r#"ip("192.168.0.1").isGlobalUnicast()"#, 1)]
#[case(r#"ip("10.0.0.1").isGlobalUnicast()"#, 1)]
#[case(r#"ip("2001:db8::abcd").isGlobalUnicast()"#, 1)]
#[case(r#"ip("0.0.0.0").isGlobalUnicast()"#, 0)]
#[case(r#"ip("255.255.255.255").isGlobalUnicast()"#, 0)]
#[case(r#"ip("127.0.0.1").isGlobalUnicast()"#, 0)]
#[case(r#"ip("::1").isGlobalUnicast()"#, 0)]
#[case(r#"ip("ff00::1").isGlobalUnicast()"#, 0)]
fn test_ip_is_global_unicast(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}

// ── ip() serialises to string ─────────────────────────────────────────────────

#[test]
fn test_ip_serializes_to_string() {
    let result = compile_and_execute_string(r#"string(ip("127.0.0.1"))"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, "127.0.0.1");
}

#[test]
fn test_ipv6_serializes_to_canonical_string() {
    let result = compile_and_execute_string(r#"string(ip("2001:db8::abcd"))"#)
        .expect("Failed to compile and execute");
    assert_eq!(result, "2001:db8::abcd");
}

// ── ip() equality ─────────────────────────────────────────────────────────────

#[rstest]
#[case(r#"ip("127.0.0.1") == ip("127.0.0.1")"#, 1)]
#[case(r#"ip("127.0.0.1") == ip("10.0.0.1")"#, 0)]
#[case(r#"ip("2001:db8::1") == ip("2001:DB8::1")"#, 1)] // IPv6 normalised on parse
#[case(r#"ip("::") == ip("::ffff")"#, 0)]
#[case(r#"ip("::ffff:c0a8:1") == ip("192.168.0.1")"#, 1)] // cross-family: IPv4-mapped IPv6 == IPv4
#[case(r#"ip("::ffff:c0a8:1") == ip("192.168.10.1")"#, 0)]
fn test_ip_equality(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' expected {}",
        expr, expected
    );
}
