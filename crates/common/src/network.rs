// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Network utility functions

use std::net::IpAddr;

/// Check if a host address is a loopback address
/// Supports IPv4 (127.0.0.1, 127.x.x.x), IPv6 (::1), and hostname (localhost)
pub fn is_loopback_address(host: &str) -> bool {
    // Handle "localhost" as special case
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }

    // Try parsing as IpAddr (handles "127.0.0.1", "::1", etc.)
    if let Ok(ip) = host.parse::<IpAddr>() {
        return ip.is_loopback();
    }

    // Fail-safe: if we can't parse it, assume non-loopback for security
    false
}

/// Validate that a string is a valid IP address (IPv4 or IPv6) or hostname
///
/// # Examples
/// ```
/// use ssh_tunnel_common::is_valid_host;
///
/// assert!(is_valid_host("192.168.1.1"));
/// assert!(is_valid_host("::1"));
/// assert!(is_valid_host("2001:db8::1"));
/// assert!(is_valid_host("example.com"));
/// assert!(is_valid_host("my-server.local"));
/// assert!(!is_valid_host(""));
/// assert!(!is_valid_host("-invalid.com"));
/// assert!(!is_valid_host("10.1.2.256"));  // Invalid IP octet
/// ```
pub fn is_valid_host(host: &str) -> bool {
    // Try parsing as IP address first
    if host.parse::<IpAddr>().is_ok() {
        return true;
    }

    // Check if this looks like an IPv4 address (4 numeric parts separated by dots)
    // If so, reject it since it failed IP parsing above
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
        // Looks like IPv4 but failed parsing - invalid IP
        return false;
    }

    // Validate as hostname/domain name
    // Basic validation: alphanumeric, dots, hyphens
    // RFC 1123 hostname rules
    if host.is_empty() || host.len() > 253 {
        return false;
    }

    // Split by dots and validate each label
    host.split('.')
        .all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_alphanumeric() || c == '-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_loopback_address() {
        // IPv4 loopback
        assert!(is_loopback_address("127.0.0.1"));
        assert!(is_loopback_address("127.0.0.2"));
        assert!(is_loopback_address("127.255.255.255"));

        // IPv6 loopback
        assert!(is_loopback_address("::1"));

        // Hostname
        assert!(is_loopback_address("localhost"));
        assert!(is_loopback_address("LOCALHOST"));
        assert!(is_loopback_address("LocalHost"));

        // Non-loopback addresses
        assert!(!is_loopback_address("0.0.0.0"));
        assert!(!is_loopback_address("192.168.1.1"));
        assert!(!is_loopback_address("10.0.0.1"));
        assert!(!is_loopback_address("example.com"));
        assert!(!is_loopback_address("::"));
        assert!(!is_loopback_address("::2"));
    }

    #[test]
    fn test_is_valid_host() {
        // Valid IPv4 addresses
        assert!(is_valid_host("192.168.1.1"));
        assert!(is_valid_host("10.0.0.1"));
        assert!(is_valid_host("127.0.0.1"));
        assert!(is_valid_host("0.0.0.0"));
        assert!(is_valid_host("255.255.255.255"));

        // Invalid IPv4 addresses (out of range octets)
        assert!(!is_valid_host("10.1.2.256"));  // 256 > 255
        assert!(!is_valid_host("256.1.1.1"));   // First octet > 255
        assert!(!is_valid_host("1.1.1.999"));   // Last octet > 255
        assert!(!is_valid_host("300.300.300.300"));  // All octets > 255

        // Valid IPv6 addresses
        assert!(is_valid_host("::1"));
        assert!(is_valid_host("::"));
        assert!(is_valid_host("2001:db8::1"));
        assert!(is_valid_host("fe80::1"));

        // Valid hostnames
        assert!(is_valid_host("localhost"));
        assert!(is_valid_host("example.com"));
        assert!(is_valid_host("my-server.local"));
        assert!(is_valid_host("server-01.example.com"));
        assert!(is_valid_host("a.b.c.d.e.f"));

        // Invalid hostnames
        assert!(!is_valid_host(""));  // Empty
        assert!(!is_valid_host("-invalid.com"));  // Starts with hyphen
        assert!(!is_valid_host("invalid-.com"));  // Ends with hyphen
        assert!(!is_valid_host("invalid..com"));  // Double dot
        assert!(!is_valid_host(".invalid.com"));  // Starts with dot
        assert!(!is_valid_host("invalid.com."));  // Ends with dot (actually invalid in our validator)
        assert!(!is_valid_host("in valid.com"));  // Contains space
        assert!(!is_valid_host("invalid_host.com"));  // Contains underscore
    }
}
