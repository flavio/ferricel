//! Kubernetes CEL IP address library extensions.
//!
//! Implements the Kubernetes IP address functions described in:
//!   <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-ip-address-library>
//!
//! Functions:
//!   - `isIP(string)`                    → bool
//!   - `ip(string)`                      → IpAddr (or error if invalid)
//!   - `ip.isCanonical(string)`          → bool  (global function on string)
//!   - `<IP>.family()`                   → int   (4 or 6)
//!   - `<IP>.isUnspecified()`            → bool
//!   - `<IP>.isLoopback()`               → bool
//!   - `<IP>.isLinkLocalMulticast()`     → bool
//!   - `<IP>.isLinkLocalUnicast()`       → bool
//!   - `<IP>.isGlobalUnicast()`          → bool
//!
//! Constraints (matching Kubernetes spec):
//!   - IPv4-mapped IPv6 addresses (e.g. `::ffff:1.2.3.4`) are NOT allowed.
//!   - IP addresses with zones (e.g. `fe80::1%eth0`) are NOT allowed.
//!   - Leading zeros in IPv4 octets (e.g. `010.0.0.1`) are NOT allowed.

use crate::error::create_error_value;
use crate::types::CelValue;
use slog::error;
use std::net::IpAddr;

// ─────────────────────────────────────────────────────────────────────────────
// Validation helper
// ─────────────────────────────────────────────────────────────────────────────

/// Parses and validates a string as a Kubernetes-compliant IP address.
///
/// Returns `Ok(IpAddr)` on success, `Err(message)` on failure.
///
/// Kubernetes-specific rejections (beyond basic IP parsing):
/// - Zone IDs (`%` anywhere in the string).
/// - IPv4-mapped IPv6 addresses written with dotted-quad notation
///   (`::ffff:192.168.0.1`). Pure-hex forms like `::ffff:c0a8:1` are allowed,
///   matching the CEL conformance spec which permits cross-family equality
///   between such addresses and their IPv4 equivalents.
/// - Leading zeros in any IPv4 octet (`010.0.0.1`).
fn parse_k8s_ip(s: &str) -> Result<IpAddr, String> {
    // Reject zone IDs — present before we even try to parse.
    if s.contains('%') {
        return Err("IP Address with zone value is not allowed".to_string());
    }

    // Reject IPv4-mapped IPv6 in dotted-quad notation (e.g. `::ffff:1.2.3.4`).
    // This check must happen on the raw string before parsing, because Rust
    // normalises both dotted and hex forms into the same internal representation.
    // Pure-hex forms like `::ffff:c0a8:1` are permitted per the CEL spec.
    let is_ipv6_with_dotted_component = s.contains(':')
        && s.split(':')
            .next_back()
            .map(|part| part.contains('.'))
            .unwrap_or(false);
    if is_ipv6_with_dotted_component {
        return Err("IPv4-mapped IPv6 address is not allowed".to_string());
    }

    let addr: IpAddr = s.parse().map_err(|_| {
        format!(
            "IP Address '{}' parse error during conversion from string",
            s
        )
    })?;

    // Reject leading zeros in IPv4 octets.
    // Rust's parser already rejects them on modern versions, but we enforce
    // this explicitly for clarity and forward-compatibility.
    if addr.is_ipv4() {
        for octet_str in s.split('.') {
            if octet_str.len() > 1 && octet_str.starts_with('0') {
                return Err(format!(
                    "IP Address '{}' parse error during conversion from string",
                    s
                ));
            }
        }
    }

    Ok(addr)
}

// ─────────────────────────────────────────────────────────────────────────────
// ip()
// ─────────────────────────────────────────────────────────────────────────────

/// Converts a string to an IP address. Returns an error value if the string is
/// not a valid Kubernetes-compliant IP address.
///
/// # Safety
/// `str_ptr` must be a valid, non-null pointer to a `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_ip_parse(str_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if str_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_ip_parse");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*str_ptr };
    let s = match val {
        CelValue::String(s) => s.as_str(),
        other => {
            error!(log, "expected String"; "function" => "cel_k8s_ip_parse", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    match parse_k8s_ip(s) {
        Ok(addr) => Box::into_raw(Box::new(CelValue::IpAddr(addr))),
        Err(msg) => {
            error!(log, "invalid IP address"; "function" => "cel_k8s_ip_parse", "error" => &msg);
            create_error_value(&msg)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// isIP()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the string is a valid Kubernetes-compliant IP address.
///
/// # Safety
/// `str_ptr` must be a valid, non-null pointer to a `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_is_ip(str_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if str_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_is_ip");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*str_ptr };
    let s = match val {
        CelValue::String(s) => s.as_str(),
        other => {
            error!(log, "expected String"; "function" => "cel_k8s_is_ip", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    let ok = parse_k8s_ip(s).is_ok();
    Box::into_raw(Box::new(CelValue::Bool(ok)))
}

// ─────────────────────────────────────────────────────────────────────────────
// ip.isCanonical()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the string is a valid IP address AND is in canonical form.
///
/// Canonical form means the string equals `addr.to_string()`:
/// - All valid IPv4 addresses are canonical (Rust always renders them in
///   decimal dotted-quad notation).
/// - IPv6 addresses must be lowercase and use `::` compression as applied by
///   Rust's `IpAddr::to_string()`.
///
/// # Safety
/// `str_ptr` must be a valid, non-null pointer to a `CelValue`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_ip_is_canonical(str_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if str_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_ip_is_canonical");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*str_ptr };
    let s = match val {
        CelValue::String(s) => s.as_str(),
        other => {
            error!(log, "expected String"; "function" => "cel_k8s_ip_is_canonical", "got" => format!("{:?}", other));
            return create_error_value("no such overload");
        }
    };

    match parse_k8s_ip(s) {
        Ok(addr) => Box::into_raw(Box::new(CelValue::Bool(addr.to_string() == s))),
        Err(msg) => {
            error!(log, "invalid IP address"; "function" => "cel_k8s_ip_is_canonical", "error" => &msg);
            create_error_value(&msg)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// family()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the IP address family as an integer: `4` for IPv4, `6` for IPv6.
///
/// # Safety
/// `ip_ptr` must be a valid, non-null pointer to a `CelValue::IpAddr`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_ip_family(ip_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if ip_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_ip_family");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*ip_ptr };
    match val {
        CelValue::IpAddr(addr) => {
            let family: i64 = if addr.is_ipv4() { 4 } else { 6 };
            Box::into_raw(Box::new(CelValue::Int(family)))
        }
        other => {
            error!(log, "expected IpAddr"; "function" => "cel_k8s_ip_family", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// isUnspecified()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the IP address is the unspecified address:
/// `0.0.0.0` (IPv4) or `::` (IPv6).
///
/// # Safety
/// `ip_ptr` must be a valid, non-null pointer to a `CelValue::IpAddr`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_ip_is_unspecified(ip_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if ip_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_ip_is_unspecified");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*ip_ptr };
    match val {
        CelValue::IpAddr(addr) => Box::into_raw(Box::new(CelValue::Bool(addr.is_unspecified()))),
        other => {
            error!(log, "expected IpAddr"; "function" => "cel_k8s_ip_is_unspecified", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// isLoopback()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the IP address is a loopback address:
/// `127.x.x.x` (IPv4) or `::1` (IPv6).
///
/// # Safety
/// `ip_ptr` must be a valid, non-null pointer to a `CelValue::IpAddr`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_ip_is_loopback(ip_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if ip_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_ip_is_loopback");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*ip_ptr };
    match val {
        CelValue::IpAddr(addr) => Box::into_raw(Box::new(CelValue::Bool(addr.is_loopback()))),
        other => {
            error!(log, "expected IpAddr"; "function" => "cel_k8s_ip_is_loopback", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// isLinkLocalMulticast()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the IP address is a link-local multicast address:
/// `224.0.0.x` (IPv4) or `ff00::/8` (IPv6).
///
/// # Safety
/// `ip_ptr` must be a valid, non-null pointer to a `CelValue::IpAddr`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_ip_is_link_local_multicast(
    ip_ptr: *mut CelValue,
) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if ip_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_ip_is_link_local_multicast");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*ip_ptr };
    match val {
        CelValue::IpAddr(addr) => {
            // IPv4 link-local multicast: 224.0.0.0/24 (i.e. 224.0.0.x)
            // IPv6 link-local multicast: ff00::/8 (first byte 0xff)
            // This matches Go's net.IP.IsLinkLocalMulticast behavior.
            let result = match addr {
                IpAddr::V4(v4) => {
                    let octets = v4.octets();
                    octets[0] == 224 && octets[1] == 0 && octets[2] == 0
                }
                IpAddr::V6(v6) => v6.segments()[0] & 0xff00 == 0xff00,
            };
            Box::into_raw(Box::new(CelValue::Bool(result)))
        }
        other => {
            error!(log, "expected IpAddr"; "function" => "cel_k8s_ip_is_link_local_multicast", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// isLinkLocalUnicast()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the IP address is a link-local unicast address:
/// `169.254.x.x` (IPv4) or `fe80::/10` (IPv6).
///
/// # Safety
/// `ip_ptr` must be a valid, non-null pointer to a `CelValue::IpAddr`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_ip_is_link_local_unicast(ip_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if ip_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_ip_is_link_local_unicast");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*ip_ptr };
    match val {
        CelValue::IpAddr(addr) => {
            let result = match addr {
                IpAddr::V4(v4) => v4.is_link_local(),
                IpAddr::V6(v6) => {
                    // fe80::/10 — first 10 bits are 1111111010
                    let seg0 = v6.segments()[0];
                    (seg0 & 0xffc0) == 0xfe80
                }
            };
            Box::into_raw(Box::new(CelValue::Bool(result)))
        }
        other => {
            error!(log, "expected IpAddr"; "function" => "cel_k8s_ip_is_link_local_unicast", "got" => format!("{:?}", other));
            create_error_value("no such overload")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// isGlobalUnicast()
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the IP address is a global unicast address.
///
/// Per the Kubernetes spec (matching Go's `net.IP.IsGlobalUnicast`):
/// - IPv4: not `0.0.0.0`, not `255.255.255.255`, and not a link-local,
///   loopback, or multicast address.
/// - IPv6: not a loopback (`::1`), link-local unicast (`fe80::/10`), or
///   multicast (`ff00::/8`) address, and not the unspecified address (`::`).
///
/// # Safety
/// `ip_ptr` must be a valid, non-null pointer to a `CelValue::IpAddr`.
#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cel_k8s_ip_is_global_unicast(ip_ptr: *mut CelValue) -> *mut CelValue {
    let log = crate::logging::get_logger();

    if ip_ptr.is_null() {
        error!(log, "null pointer"; "function" => "cel_k8s_ip_is_global_unicast");
        return create_error_value("no such overload");
    }

    let val = unsafe { &*ip_ptr };
    match val {
        CelValue::IpAddr(addr) => {
            // Matches Go's net.IP.IsGlobalUnicast():
            // not unspecified, not loopback, not multicast, not link-local,
            // and not IPv4 broadcast (255.255.255.255).
            let result = match addr {
                IpAddr::V4(v4) => {
                    !v4.is_unspecified()
                        && !v4.is_loopback()
                        && !v4.is_multicast()
                        && !v4.is_link_local()
                        && !v4.is_broadcast()
                }
                IpAddr::V6(v6) => {
                    let seg0 = v6.segments()[0];
                    let is_loopback = *v6 == std::net::Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1);
                    let is_unspecified = *v6 == std::net::Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0);
                    let is_link_local_unicast = (seg0 & 0xffc0) == 0xfe80;
                    let is_multicast = (seg0 & 0xff00) == 0xff00;
                    !is_loopback && !is_unspecified && !is_link_local_unicast && !is_multicast
                }
            };
            Box::into_raw(Box::new(CelValue::Bool(result)))
        }
        other => {
            error!(log, "expected IpAddr"; "function" => "cel_k8s_ip_is_global_unicast", "got" => format!("{:?}", other));
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

    unsafe fn make_ip(s: &str) -> *mut CelValue {
        let str_ptr = unsafe { make_str(s) };
        unsafe { cel_k8s_ip_parse(str_ptr) }
    }

    // ── ip() / isIP() ────────────────────────────────────────────────────────

    #[rstest]
    #[case::ipv4("127.0.0.1")]
    #[case::ipv4_zero("0.0.0.0")]
    #[case::ipv4_broadcast("255.255.255.255")]
    #[case::ipv6_loopback("::1")]
    #[case::ipv6_unspecified("::")]
    #[case::ipv6_full("2001:db8::abcd")]
    #[case::ipv6_hex_mapped("::ffff:c0a8:1")]
    fn test_ip_parse_valid(#[case] input: &str) {
        let str_ptr = unsafe { make_str(input) };
        let result = unsafe { read_val(cel_k8s_ip_parse(str_ptr)) };
        assert!(
            matches!(result, CelValue::IpAddr(_)),
            "expected IpAddr for {:?}, got {:?}",
            input,
            result
        );
        unsafe { drop(Box::from_raw(str_ptr)) };
    }

    #[rstest]
    #[case::invalid("not-an-ip")]
    #[case::zone_id("fe80::1%eth0")]
    #[case::ipv4_mapped_v6_dotted("::ffff:1.2.3.4")]
    #[case::leading_zero_octet("010.0.0.1")]
    #[case::too_many_octets("1.2.3.4.5")]
    #[case::empty("")]
    fn test_ip_parse_invalid(#[case] input: &str) {
        let str_ptr = unsafe { make_str(input) };
        let result = unsafe { read_val(cel_k8s_ip_parse(str_ptr)) };
        assert!(
            matches!(result, CelValue::Error(_)),
            "expected Error for {:?}, got {:?}",
            input,
            result
        );
        unsafe { drop(Box::from_raw(str_ptr)) };
    }

    #[rstest]
    #[case::ipv4("127.0.0.1", true)]
    #[case::ipv6("::1", true)]
    #[case::zone_id("fe80::1%eth0", false)]
    #[case::mapped_dotted("::ffff:1.2.3.4", false)]
    #[case::mapped_hex("::ffff:c0a8:1", true)]
    #[case::leading_zero("010.0.0.1", false)]
    #[case::garbage("not-an-ip", false)]
    fn test_is_ip(#[case] input: &str, #[case] expected: bool) {
        let str_ptr = unsafe { make_str(input) };
        let result = unsafe { read_val(cel_k8s_is_ip(str_ptr)) };
        assert_eq!(result, CelValue::Bool(expected), "isIP({:?})", input);
        unsafe { drop(Box::from_raw(str_ptr)) };
    }

    // ── ip.isCanonical() ─────────────────────────────────────────────────────

    #[rstest]
    #[case::ipv4_canonical("127.0.0.1", true)]
    #[case::ipv4_all_zeros("0.0.0.0", true)]
    #[case::ipv4_broadcast("255.255.255.255", true)]
    #[case::ipv6_canonical("2001:db8::abcd", true)]
    #[case::ipv6_uppercase("2001:DB8::ABCD", false)]
    #[case::ipv6_uncompressed("2001:db8::0:0:0:abcd", false)]
    fn test_is_canonical_valid(#[case] input: &str, #[case] expected: bool) {
        let str_ptr = unsafe { make_str(input) };
        let result = unsafe { read_val(cel_k8s_ip_is_canonical(str_ptr)) };
        assert_eq!(
            result,
            CelValue::Bool(expected),
            "ip.isCanonical({:?})",
            input
        );
        unsafe { drop(Box::from_raw(str_ptr)) };
    }

    #[test]
    fn test_is_canonical_invalid_returns_error() {
        let str_ptr = unsafe { make_str("not-an-ip") };
        let result = unsafe { read_val(cel_k8s_ip_is_canonical(str_ptr)) };
        assert!(
            matches!(result, CelValue::Error(_)),
            "expected Error for invalid input, got {:?}",
            result
        );
        unsafe { drop(Box::from_raw(str_ptr)) };
    }

    // ── family() ─────────────────────────────────────────────────────────────

    #[rstest]
    #[case::ipv4("127.0.0.1", 4)]
    #[case::ipv4_zero("0.0.0.0", 4)]
    #[case::ipv6("::1", 6)]
    #[case::ipv6_full("2001:db8::abcd", 6)]
    fn test_family(#[case] ip_str: &str, #[case] expected: i64) {
        let ip_ptr = unsafe { make_ip(ip_str) };
        let result = unsafe { read_val(cel_k8s_ip_family(ip_ptr)) };
        assert_eq!(result, CelValue::Int(expected), "family({:?})", ip_str);
    }

    // ── isUnspecified() ───────────────────────────────────────────────────────

    #[rstest]
    #[case::ipv4_unspecified("0.0.0.0", true)]
    #[case::ipv4_loopback("127.0.0.1", false)]
    #[case::ipv6_unspecified("::", true)]
    #[case::ipv6_loopback("::1", false)]
    fn test_is_unspecified(#[case] ip_str: &str, #[case] expected: bool) {
        let ip_ptr = unsafe { make_ip(ip_str) };
        let result = unsafe { read_val(cel_k8s_ip_is_unspecified(ip_ptr)) };
        assert_eq!(
            result,
            CelValue::Bool(expected),
            "isUnspecified({:?})",
            ip_str
        );
    }

    // ── isLoopback() ──────────────────────────────────────────────────────────

    #[rstest]
    #[case::ipv4_loopback("127.0.0.1", true)]
    #[case::ipv4_loopback_other("127.1.2.3", true)]
    #[case::ipv4_non_loopback("192.168.0.1", false)]
    #[case::ipv6_loopback("::1", true)]
    #[case::ipv6_non_loopback("2001:db8::abcd", false)]
    fn test_is_loopback(#[case] ip_str: &str, #[case] expected: bool) {
        let ip_ptr = unsafe { make_ip(ip_str) };
        let result = unsafe { read_val(cel_k8s_ip_is_loopback(ip_ptr)) };
        assert_eq!(result, CelValue::Bool(expected), "isLoopback({:?})", ip_str);
    }

    // ── isLinkLocalMulticast() ────────────────────────────────────────────────

    #[rstest]
    #[case::ipv4_llmc("224.0.0.1", true)]
    #[case::ipv4_not_llmc("224.0.1.1", false)]
    #[case::ipv4_unicast("192.168.0.1", false)]
    #[case::ipv6_llmc("ff02::1", true)]
    #[case::ipv6_not_llmc("fd00::1", false)]
    fn test_is_link_local_multicast(#[case] ip_str: &str, #[case] expected: bool) {
        let ip_ptr = unsafe { make_ip(ip_str) };
        let result = unsafe { read_val(cel_k8s_ip_is_link_local_multicast(ip_ptr)) };
        assert_eq!(
            result,
            CelValue::Bool(expected),
            "isLinkLocalMulticast({:?})",
            ip_str
        );
    }

    // ── isLinkLocalUnicast() ──────────────────────────────────────────────────

    #[rstest]
    #[case::ipv4_llu("169.254.169.254", true)]
    #[case::ipv4_not_llu("192.168.0.1", false)]
    #[case::ipv6_llu("fe80::1", true)]
    #[case::ipv6_not_llu("fd80::1", false)]
    fn test_is_link_local_unicast(#[case] ip_str: &str, #[case] expected: bool) {
        let ip_ptr = unsafe { make_ip(ip_str) };
        let result = unsafe { read_val(cel_k8s_ip_is_link_local_unicast(ip_ptr)) };
        assert_eq!(
            result,
            CelValue::Bool(expected),
            "isLinkLocalUnicast({:?})",
            ip_str
        );
    }

    // ── isGlobalUnicast() ─────────────────────────────────────────────────────

    #[rstest]
    #[case::ipv4_global("192.168.0.1", true)]
    #[case::ipv4_broadcast("255.255.255.255", false)]
    #[case::ipv4_unspecified("0.0.0.0", false)]
    #[case::ipv6_global("2001:db8::abcd", true)]
    #[case::ipv6_multicast("ff00::1", false)]
    fn test_is_global_unicast(#[case] ip_str: &str, #[case] expected: bool) {
        let ip_ptr = unsafe { make_ip(ip_str) };
        let result = unsafe { read_val(cel_k8s_ip_is_global_unicast(ip_ptr)) };
        assert_eq!(
            result,
            CelValue::Bool(expected),
            "isGlobalUnicast({:?})",
            ip_str
        );
    }

    // ── parse_k8s_ip (private helper) ────────────────────────────────────────

    #[rstest]
    #[case::ipv4("127.0.0.1")]
    #[case::ipv4_zero("0.0.0.0")]
    #[case::ipv4_broadcast("255.255.255.255")]
    #[case::ipv6_loopback("::1")]
    #[case::ipv6_unspecified("::")]
    #[case::ipv6_full("2001:db8::abcd")]
    #[case::ipv6_hex_mapped("::ffff:c0a8:1")]
    fn test_parse_k8s_ip_valid(#[case] input: &str) {
        assert!(parse_k8s_ip(input).is_ok(), "expected Ok for {:?}", input);
    }

    #[test]
    fn test_parse_k8s_ip_zone_id_rejected() {
        let err = parse_k8s_ip("fe80::1%eth0").unwrap_err();
        assert_eq!(err, "IP Address with zone value is not allowed");
    }

    #[test]
    fn test_parse_k8s_ip_dotted_mapped_rejected() {
        let err = parse_k8s_ip("::ffff:1.2.3.4").unwrap_err();
        assert_eq!(err, "IPv4-mapped IPv6 address is not allowed");
    }

    #[rstest]
    #[case::leading_zero_first("010.0.0.1")]
    #[case::leading_zero_second("192.001.0.1")]
    fn test_parse_k8s_ip_leading_zero_rejected(#[case] input: &str) {
        let err = parse_k8s_ip(input).unwrap_err();
        assert_eq!(
            err,
            format!(
                "IP Address '{}' parse error during conversion from string",
                input
            )
        );
    }

    #[rstest]
    #[case::garbage("not-an-ip")]
    #[case::empty("")]
    #[case::too_many_octets("1.2.3.4.5")]
    fn test_parse_k8s_ip_invalid_rejected(#[case] input: &str) {
        assert!(parse_k8s_ip(input).is_err(), "expected Err for {:?}", input);
    }

    // ── wrong type returns error ──────────────────────────────────────────────

    #[test]
    fn test_family_wrong_type_returns_error() {
        let val_ptr = unsafe { make_val(CelValue::Int(42)) };
        let result = unsafe { read_val(cel_k8s_ip_family(val_ptr)) };
        assert!(matches!(result, CelValue::Error(_)));
        unsafe { drop(Box::from_raw(val_ptr)) };
    }

    #[test]
    fn test_is_unspecified_wrong_type_returns_error() {
        let val_ptr = unsafe { make_val(CelValue::Bool(true)) };
        let result = unsafe { read_val(cel_k8s_ip_is_unspecified(val_ptr)) };
        assert!(matches!(result, CelValue::Error(_)));
        unsafe { drop(Box::from_raw(val_ptr)) };
    }
}
