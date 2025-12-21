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
}
