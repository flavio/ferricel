//! Kubernetes CEL CIDR library extensions.
//!
//! Implements the Kubernetes CIDR functions described in:
//!   <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-cidr-library>
//!
//! Functions:
//!   - `isCIDR(string)`                  → bool
//!   - `cidr(string)`                    → Cidr (or error if invalid)
//!   - `<CIDR>.ip()`                     → IpAddr (network address part)
//!   - `<CIDR>.masked()`                 → Cidr  (canonical CIDR, host bits zeroed)
//!   - `<CIDR>.prefixLength()`           → int
//!   - `<CIDR>.containsIP(IP)`           → bool  (IP object or string)
//!   - `<CIDR>.containsCIDR(CIDR)`       → bool  (CIDR object or string)
//!
//! Constraints (matching Kubernetes spec):
//!   - IPv4-mapped IPv6 in dotted-quad notation is NOT allowed.
//!   - CIDR with zone IDs is NOT allowed.
//!   - Leading zeros in IPv4 octets are NOT allowed.
//!   - Host bits in the address part do NOT need to be zero (masked() zeroes them).

use crate::error::{create_error_value, null_to_unbound};
use crate::types::CelValue;
use slog::error;
use std::net::IpAddr;

// ─────────────────────────────────────────────────────────────────────────────
// Validation / parsing helper
// ─────────────────────────────────────────────────────────────────────────────

/// Parses and validates a string as a Kubernetes-compliant CIDR block.
///
/// Returns `Ok((IpAddr, prefix_len))` on success, `Err(message)` on failure.
///
/// The returned `IpAddr` is the address as given (host bits are NOT zeroed).
/// Call `apply_mask` to produce the network address.
fn parse_k8s_cidr(s: &str) -> Result<(IpAddr, u8), String> {
    // Reject zone IDs.
    if s.contains('%') {
        return Err("CIDR with zone value is not allowed".to_string());
    }

    // Split on '/'.
    let slash_pos = s
        .rfind('/')
        .ok_or_else(|| "network address parse error during conversion from string".to_string())?;
    let ip_part = &s[..slash_pos];
    let prefix_part = &s[slash_pos + 1..];

    // Parse prefix length — must be a non-empty decimal integer.
    if prefix_part.is_empty() {
        return Err("network address parse error during conversion from string".to_string());
    }
    let prefix_len: u8 = prefix_part
        .parse()
        .map_err(|_| "network address parse error during conversion from string".to_string())?;

    // Reject IPv4-mapped IPv6 in dotted-quad notation.
    let is_ipv6_with_dotted = ip_part.contains(':')
        && ip_part
            .split(':')
            .next_back()
            .map(|part| part.contains('.'))
            .unwrap_or(false);
    if is_ipv6_with_dotted {
        return Err("IPv4-mapped IPv6 address is not allowed".to_string());
    }

    // Parse the IP address part.
    let addr: IpAddr = ip_part
        .parse()
        .map_err(|_| "network address parse error during conversion from string".to_string())?;

    // Validate prefix length range.
    let max_prefix = if addr.is_ipv4() { 32u8 } else { 128u8 };
    if prefix_len > max_prefix {
        return Err("network address parse error during conversion from string".to_string());
    }

    // Reject leading zeros in IPv4 octets.
    if addr.is_ipv4() {
        for octet_str in ip_part.split('.') {
            if octet_str.len() > 1 && octet_str.starts_with('0') {
                return Err("network address parse error during conversion from string".to_string());
            }
        }
    }

    Ok((addr, prefix_len))
}

// ─────────────────────────────────────────────────────────────────────────────
// Masking helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Apply the prefix mask to an IPv4 address, returning the network address.
fn mask_ipv4(addr: std::net::Ipv4Addr, prefix_len: u8) -> std::net::Ipv4Addr {
    let bits = u32::from(addr);
    let masked = if prefix_len == 0 {
        0u32
    } else {
        bits & (!0u32 << (32 - prefix_len))
    };
    std::net::Ipv4Addr::from(masked)
}

/// Apply the prefix mask to an IPv6 address, returning the network address.
fn mask_ipv6(addr: std::net::Ipv6Addr, prefix_len: u8) -> std::net::Ipv6Addr {
    let bits = u128::from(addr);
    let masked = if prefix_len == 0 {
        0u128
    } else {
        bits & (!0u128 << (128 - prefix_len))
    };
    std::net::Ipv6Addr::from(masked)
}

/// Return the network address (host bits zeroed).
fn apply_mask(addr: IpAddr, prefix_len: u8) -> IpAddr {
    match addr {
        IpAddr::V4(v4) => IpAddr::V4(mask_ipv4(v4, prefix_len)),
        IpAddr::V6(v6) => IpAddr::V6(mask_ipv6(v6, prefix_len)),
    }
}

/// Check whether `ip` is contained within the CIDR block `(network, prefix_len)`.
/// `network` must already be the masked network address.
fn cidr_contains_ip(network: IpAddr, prefix_len: u8, ip: IpAddr) -> bool {
    match (network, ip) {
        (IpAddr::V4(net4), IpAddr::V4(ip4)) => mask_ipv4(ip4, prefix_len) == net4,
        (IpAddr::V6(net6), IpAddr::V6(ip6)) => mask_ipv6(ip6, prefix_len) == net6,
        _ => false,
    }
}

/// Check whether CIDR `(other_net, other_prefix)` is a subnet of `(network, prefix_len)`.
fn cidr_contains_cidr(
    network: IpAddr,
    prefix_len: u8,
    other_net: IpAddr,
    other_prefix: u8,
) -> bool {
    if other_prefix < prefix_len {
        return false;
    }
    cidr_contains_ip(network, prefix_len, other_net)
}

// ─────────────────────────────────────────────────────────────────────────────
// cidr()
// ─────────────────────────────────────────────────────────────────────────────

/// Converts a string to a CIDR value. Returns an error if invalid.
///
/// # Safety
/// `str_ptr` must be a valid, non-null pointer to a `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_cidr_parse(str_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if str_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_cidr_parse");
        return create_error_value("no such overload");
    }

    let val = unsafe { null_to_unbound(str_ptr) };
    let s = match val {
        CelValue::String(ref s) => s.clone(),
        other => {
            error!(log, "expected String"; "function" => "cel_k8s_cidr_parse", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    match parse_k8s_cidr(&s) {
        Ok((addr, prefix_len)) => Box::into_raw(Box::new(CelValue::Cidr(addr, prefix_len))),
        Err(msg) => {
            error!(log, "invalid CIDR"; "function" => "cel_k8s_cidr_parse", "error" => &msg);
            create_error_value(&msg)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// isCIDR()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the string is a valid Kubernetes-compliant CIDR block.
///
/// # Safety
/// `str_ptr` must be a valid, non-null pointer to a `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_is_cidr(str_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if str_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_is_cidr");
        return create_error_value("no such overload");
    }

    let val = unsafe { null_to_unbound(str_ptr) };
    let s = match val {
        CelValue::String(ref s) => s.clone(),
        other => {
            error!(log, "expected String"; "function" => "cel_k8s_is_cidr", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let ok = parse_k8s_cidr(&s).is_ok();
    Box::into_raw(Box::new(CelValue::Bool(ok)))
}

// ─────────────────────────────────────────────────────────────────────────────
// cidr.ip()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the IP address part of the CIDR block (as a `CelValue::IpAddr`).
///
/// # Safety
/// `cidr_ptr` must be a valid, non-null pointer to a `CelValue::Cidr`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_cidr_ip(cidr_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if cidr_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_cidr_ip");
        return create_error_value("no such overload");
    }

    let val = unsafe { null_to_unbound(cidr_ptr) };
    match val {
        CelValue::Cidr(addr, _) => Box::into_raw(Box::new(CelValue::IpAddr(addr))),
        other => {
            error!(log, "expected Cidr"; "function" => "cel_k8s_cidr_ip", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// cidr.masked()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the canonical CIDR block with host bits zeroed.
///
/// # Safety
/// `cidr_ptr` must be a valid, non-null pointer to a `CelValue::Cidr`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_cidr_masked(cidr_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if cidr_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_cidr_masked");
        return create_error_value("no such overload");
    }

    let val = unsafe { null_to_unbound(cidr_ptr) };
    match val {
        CelValue::Cidr(addr, prefix_len) => {
            let network = apply_mask(addr, prefix_len);
            Box::into_raw(Box::new(CelValue::Cidr(network, prefix_len)))
        }
        other => {
            error!(log, "expected Cidr"; "function" => "cel_k8s_cidr_masked", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// cidr.prefixLength()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the prefix length of the CIDR block as an integer.
///
/// # Safety
/// `cidr_ptr` must be a valid, non-null pointer to a `CelValue::Cidr`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_cidr_prefix_length(cidr_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if cidr_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_cidr_prefix_length");
        return create_error_value("no such overload");
    }

    let val = unsafe { null_to_unbound(cidr_ptr) };
    match val {
        CelValue::Cidr(_, prefix_len) => Box::into_raw(Box::new(CelValue::Int(prefix_len as i64))),
        other => {
            error!(log, "expected Cidr"; "function" => "cel_k8s_cidr_prefix_length", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// cidr.containsIP(IP object or string)
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the CIDR block contains the given IP address.
///
/// Accepts either a `CelValue::IpAddr` or a `CelValue::String` as the second argument,
/// dispatching at runtime so the compiler can use a single call site.
///
/// # Safety
/// `cidr_ptr` and `ip_ptr` must be valid, non-null pointers to their respective `CelValue`s.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_cidr_contains_ip_obj(
    cidr_ptr: *mut CelValue,
    ip_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if cidr_ptr.is_null() || ip_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_cidr_contains_ip_obj");
        return create_error_value("no such overload");
    }

    let cidr_val = unsafe { null_to_unbound(cidr_ptr) };
    let ip_val = unsafe { null_to_unbound(ip_ptr) };

    let (cidr_addr, prefix_len) = match cidr_val {
        CelValue::Cidr(a, p) => (a, p),
        _ => {
            error!(log, "expected Cidr as first argument"; "function" => "cel_k8s_cidr_contains_ip_obj");
            return create_error_value("no such overload");
        }
    };

    let ip: IpAddr = match ip_val {
        CelValue::IpAddr(a) => a,
        CelValue::String(ref s) => match s.parse() {
            Ok(a) => a,
            Err(_) => {
                let msg = format!(
                    "IP Address '{}' parse error during conversion from string",
                    s
                );
                return create_error_value(&msg);
            }
        },
        _ => {
            error!(log, "expected IpAddr or String as second argument"; "function" => "cel_k8s_cidr_contains_ip_obj");
            return create_error_value("no such overload");
        }
    };

    let network = apply_mask(cidr_addr, prefix_len);
    let result = cidr_contains_ip(network, prefix_len, ip);
    Box::into_raw(Box::new(CelValue::Bool(result)))
}

// ─────────────────────────────────────────────────────────────────────────────
// cidr.containsCIDR(CIDR object or string)
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the CIDR block contains (is a superset of) the given CIDR.
///
/// Accepts either a `CelValue::Cidr` or a `CelValue::String` as the second argument,
/// dispatching at runtime so the compiler can use a single call site.
///
/// # Safety
/// `cidr_ptr` and `other_ptr` must be valid, non-null pointers to their respective `CelValue`s.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_cidr_contains_cidr_obj(
    cidr_ptr: *mut CelValue,
    other_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if cidr_ptr.is_null() || other_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_cidr_contains_cidr_obj");
        return create_error_value("no such overload");
    }

    let cidr_val = unsafe { null_to_unbound(cidr_ptr) };
    let other_val = unsafe { null_to_unbound(other_ptr) };

    let (cidr_addr, prefix_len) = match cidr_val {
        CelValue::Cidr(a, p) => (a, p),
        _ => {
            error!(log, "expected Cidr as first argument"; "function" => "cel_k8s_cidr_contains_cidr_obj");
            return create_error_value("no such overload");
        }
    };

    let (other_addr, other_prefix) = match other_val {
        CelValue::Cidr(a, p) => (a, p),
        CelValue::String(ref s) => match parse_k8s_cidr(s) {
            Ok(v) => v,
            Err(msg) => return create_error_value(&msg),
        },
        _ => {
            error!(log, "expected Cidr or String as second argument"; "function" => "cel_k8s_cidr_contains_cidr_obj");
            return create_error_value("no such overload");
        }
    };

    let network = apply_mask(cidr_addr, prefix_len);
    let other_network = apply_mask(other_addr, other_prefix);
    let result = cidr_contains_cidr(network, prefix_len, other_network, other_prefix);
    Box::into_raw(Box::new(CelValue::Bool(result)))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{make_str, make_val, read_val};
    use super::*;
    use rstest::rstest;

    unsafe fn make_cidr(s: &str) -> *mut CelValue {
        let str_ptr = unsafe { make_str(s) };
        unsafe { cel_k8s_cidr_parse(str_ptr) }
    }

    unsafe fn make_ip(s: &str) -> *mut CelValue {
        let str_ptr = unsafe { make_str(s) };
        unsafe { super::super::ip::cel_k8s_ip_parse(str_ptr) }
    }

    // ── cidr() / isCIDR() ────────────────────────────────────────────────────

    #[rstest]
    #[case::ipv4("192.168.0.0/24")]
    #[case::ipv4_host("192.168.0.1/32")]
    #[case::ipv4_all("0.0.0.0/0")]
    #[case::ipv6("2001:db8::/32")]
    #[case::ipv6_host("2001:db8::1/128")]
    #[case::ipv4_with_host_bits("192.168.0.1/24")] // host bits non-zero: allowed by cidr()
    fn test_cidr_parse_valid(#[case] input: &str) {
        let str_ptr = unsafe { make_str(input) };
        let result = unsafe { read_val(cel_k8s_cidr_parse(str_ptr)) };
        assert!(
            matches!(result, CelValue::Cidr(_, _)),
            "expected Cidr for {:?}, got {:?}",
            input,
            result
        );
    }

    #[rstest]
    #[case::no_prefix("192.168.0.0")]
    #[case::empty_prefix("192.168.0.0/")]
    #[case::prefix_too_large("192.168.0.0/33")]
    #[case::zone_id("fe80::1%en0/24")]
    #[case::ipv4_mapped_dotted("::ffff:192.168.0.1/24")]
    #[case::leading_zero("010.0.0.0/8")]
    fn test_cidr_parse_invalid(#[case] input: &str) {
        let str_ptr = unsafe { make_str(input) };
        let result = unsafe { read_val(cel_k8s_cidr_parse(str_ptr)) };
        assert!(
            matches!(result, CelValue::Error(_)),
            "expected Error for {:?}, got {:?}",
            input,
            result
        );
    }

    #[rstest]
    #[case::valid("192.168.0.0/24", true)]
    #[case::ipv6("2001:db8::/32", true)]
    #[case::invalid("not-a-cidr", false)]
    #[case::zone("fe80::1%en0/24", false)]
    fn test_is_cidr(#[case] input: &str, #[case] expected: bool) {
        let str_ptr = unsafe { make_str(input) };
        let result = unsafe { read_val(cel_k8s_is_cidr(str_ptr)) };
        assert_eq!(result, CelValue::Bool(expected), "isCIDR({:?})", input);
    }

    // ── cidr.ip() ─────────────────────────────────────────────────────────────

    #[test]
    fn test_cidr_ip_ipv4() {
        let cidr_ptr = unsafe { make_cidr("192.168.0.0/24") };
        let result = unsafe { read_val(cel_k8s_cidr_ip(cidr_ptr)) };
        assert!(
            matches!(result, CelValue::IpAddr(IpAddr::V4(_))),
            "expected IpAddr::V4, got {:?}",
            result
        );
    }

    #[test]
    fn test_cidr_ip_ipv6() {
        let cidr_ptr = unsafe { make_cidr("2001:db8::/32") };
        let result = unsafe { read_val(cel_k8s_cidr_ip(cidr_ptr)) };
        assert!(
            matches!(result, CelValue::IpAddr(IpAddr::V6(_))),
            "expected IpAddr::V6, got {:?}",
            result
        );
    }

    // ── cidr.masked() ─────────────────────────────────────────────────────────

    #[test]
    fn test_cidr_masked_ipv4() {
        // 192.168.0.1/24 → 192.168.0.0/24
        let cidr_ptr = unsafe { make_cidr("192.168.0.1/24") };
        let result = unsafe { read_val(cel_k8s_cidr_masked(cidr_ptr)) };
        match result {
            CelValue::Cidr(addr, prefix_len) => {
                assert_eq!(addr.to_string(), "192.168.0.0");
                assert_eq!(prefix_len, 24);
            }
            other => panic!("expected Cidr, got {:?}", other),
        }
    }

    #[test]
    fn test_cidr_masked_already_canonical() {
        // 192.168.0.0/24 → 192.168.0.0/24 (unchanged)
        let cidr_ptr = unsafe { make_cidr("192.168.0.0/24") };
        let result = unsafe { read_val(cel_k8s_cidr_masked(cidr_ptr)) };
        match result {
            CelValue::Cidr(addr, prefix_len) => {
                assert_eq!(addr.to_string(), "192.168.0.0");
                assert_eq!(prefix_len, 24);
            }
            other => panic!("expected Cidr, got {:?}", other),
        }
    }

    // ── cidr.prefixLength() ──────────────────────────────────────────────────

    #[rstest]
    #[case("192.168.0.0/24", 24)]
    #[case("10.0.0.0/8", 8)]
    #[case("2001:db8::/32", 32)]
    fn test_prefix_length(#[case] cidr_str: &str, #[case] expected: i64) {
        let cidr_ptr = unsafe { make_cidr(cidr_str) };
        let result = unsafe { read_val(cel_k8s_cidr_prefix_length(cidr_ptr)) };
        assert_eq!(
            result,
            CelValue::Int(expected),
            "prefixLength({:?})",
            cidr_str
        );
    }

    // ── cidr.containsIP(IP object) ────────────────────────────────────────────

    #[rstest]
    #[case("192.168.0.0/24", "192.168.0.1", true)]
    #[case("192.168.0.0/24", "192.168.0.255", true)]
    #[case("192.168.0.0/24", "192.168.1.1", false)]
    #[case("10.0.0.0/8", "10.1.2.3", true)]
    #[case("10.0.0.0/8", "11.0.0.1", false)]
    fn test_contains_ip_obj(#[case] cidr_str: &str, #[case] ip_str: &str, #[case] expected: bool) {
        let cidr_ptr = unsafe { make_cidr(cidr_str) };
        let ip_ptr = unsafe { make_ip(ip_str) };
        let result = unsafe { read_val(cel_k8s_cidr_contains_ip_obj(cidr_ptr, ip_ptr)) };
        assert_eq!(
            result,
            CelValue::Bool(expected),
            "containsIP({:?}, {:?})",
            cidr_str,
            ip_str
        );
    }

    #[test]
    fn test_contains_ip_cross_family_returns_false() {
        let cidr_ptr = unsafe { make_cidr("192.168.0.0/24") };
        let ip_ptr = unsafe { make_ip("2001:db8::1") };
        let result = unsafe { read_val(cel_k8s_cidr_contains_ip_obj(cidr_ptr, ip_ptr)) };
        assert_eq!(result, CelValue::Bool(false));
    }

    // ── cidr.containsCIDR(CIDR object) ───────────────────────────────────────

    #[rstest]
    #[case("192.168.0.0/24", "192.168.0.0/25", true)]
    #[case("192.168.0.0/24", "192.168.0.1/32", true)]
    #[case("192.168.0.0/24", "192.168.0.0/23", false)] // superset, not subset
    #[case("10.0.0.0/8", "10.0.0.0/8", true)] // equal
    fn test_contains_cidr_obj(
        #[case] cidr_str: &str,
        #[case] other_str: &str,
        #[case] expected: bool,
    ) {
        let cidr_ptr = unsafe { make_cidr(cidr_str) };
        let other_ptr = unsafe { make_cidr(other_str) };
        let result = unsafe { read_val(cel_k8s_cidr_contains_cidr_obj(cidr_ptr, other_ptr)) };
        assert_eq!(
            result,
            CelValue::Bool(expected),
            "containsCIDR({:?}, {:?})",
            cidr_str,
            other_str
        );
    }

    // ── parse_k8s_cidr (private helper) ──────────────────────────────────────

    #[rstest]
    #[case::ipv4("192.168.0.0/24")]
    #[case::ipv4_host("192.168.0.1/32")]
    #[case::ipv4_all("0.0.0.0/0")]
    #[case::ipv6("2001:db8::/32")]
    #[case::ipv6_host("2001:db8::1/128")]
    #[case::ipv4_host_bits("192.168.0.1/24")] // host bits non-zero: allowed
    fn test_parse_k8s_cidr_valid(#[case] input: &str) {
        assert!(parse_k8s_cidr(input).is_ok(), "expected Ok for {:?}", input);
    }

    #[test]
    fn test_parse_k8s_cidr_zone_id_rejected() {
        let err = parse_k8s_cidr("fe80::1%en0/64").unwrap_err();
        assert_eq!(err, "CIDR with zone value is not allowed");
    }

    #[test]
    fn test_parse_k8s_cidr_dotted_mapped_rejected() {
        let err = parse_k8s_cidr("::ffff:192.168.0.1/24").unwrap_err();
        assert_eq!(err, "IPv4-mapped IPv6 address is not allowed");
    }

    #[rstest]
    #[case::no_slash("192.168.0.0")]
    #[case::empty_prefix("192.168.0.0/")]
    #[case::ipv4_prefix_too_large("192.168.0.0/33")]
    #[case::ipv6_prefix_too_large("2001:db8::/129")]
    #[case::garbage_ip("not-an-ip/24")]
    #[case::leading_zero("010.0.0.0/8")]
    fn test_parse_k8s_cidr_invalid(#[case] input: &str) {
        let result = parse_k8s_cidr(input);
        assert!(
            result.is_err(),
            "expected Err for {:?}, got {:?}",
            input,
            result
        );
        assert_eq!(
            result.unwrap_err(),
            "network address parse error during conversion from string"
        );
    }

    // ── wrong type returns error ──────────────────────────────────────────────

    #[test]
    fn test_cidr_ip_wrong_type_returns_error() {
        let val_ptr = unsafe { make_val(CelValue::Int(42)) };
        let result = unsafe { read_val(cel_k8s_cidr_ip(val_ptr)) };
        assert!(matches!(result, CelValue::Error(_)));
    }
}
