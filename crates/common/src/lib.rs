// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// SSH Tunnel Manager - Common Library
// Shared types, utilities, and configuration structures

pub mod config;
pub mod daemon_client;
pub mod error;
pub mod keychain;
pub mod network;
pub mod profile_manager;
pub mod sse;
pub mod tls;
pub mod types;

pub use config::{ConnectionConfig, ForwardingConfig, PasswordStorage, Profile, TunnelOptions};
pub use daemon_client::{
    add_auth_header, cli_config_snippet_exists, config_needs_ip_address, create_daemon_client,
    get_cli_config_snippet_path, start_tunnel_with_events, stop_tunnel, validate_client_config,
    validate_daemon_config, ConfigValidationResult, ConnectionMode, DaemonClientConfig,
    TunnelEventHandler, TunnelStatusResponse,
};
pub use sse::{EventListener, TunnelEvent};
pub use error::{Error, Result};
pub use keychain::{get_password, has_password, is_keychain_available, remove_password, store_password};
pub use network::{is_loopback_address, is_valid_host};
pub use profile_manager::{
    delete_profile_by_id, delete_profile_by_name, get_remote_key_setup_message,
    load_all_profiles, load_profile, load_profile_by_id, load_profile_by_name,
    prepare_profile_for_remote, profile_exists_by_id, profile_exists_by_name, profiles_dir,
    save_profile,
};
pub use tls::{create_insecure_tls_config, create_pinned_tls_config};
pub use types::{
    AuthRequest, AuthRequestType, AuthResponse, AuthType, DaemonInfo, ForwardingType,
    ProfileSourceMode, StartTunnelRequest, StartTunnelResult, TunnelDomainEvent, TunnelStatus,
};

// Re-export commonly used external types
pub use chrono::{DateTime, Utc};
pub use uuid::Uuid;

/// Format a host:port address, properly handling IPv6 literal addresses.
///
/// IPv6 addresses are detected by parsing the host string. If it's a valid IPv6 address,
/// it's wrapped in brackets. IPv4 addresses and hostnames are passed through unchanged.
///
/// # Examples
/// ```
/// use ssh_tunnel_common::format_host_port;
///
/// assert_eq!(format_host_port("127.0.0.1", 8080), "127.0.0.1:8080");
/// assert_eq!(format_host_port("example.com", 443), "example.com:443");
/// assert_eq!(format_host_port("::1", 22), "[::1]:22");
/// assert_eq!(format_host_port("2001:db8::1", 80), "[2001:db8::1]:80");
/// ```
pub fn format_host_port(host: &str, port: u16) -> String {
    use std::net::IpAddr;

    // Try to parse as IP address
    if let Ok(ip) = host.parse::<IpAddr>() {
        match ip {
            IpAddr::V6(_) => {
                // IPv6 address - must be wrapped in brackets
                format!("[{}]:{}", host, port)
            }
            IpAddr::V4(_) => {
                // IPv4 address - no brackets needed
                format!("{}:{}", host, port)
            }
        }
    } else {
        // Hostname - no brackets needed
        format!("{}:{}", host, port)
    }
}

/// Format a tunnel description based on forwarding configuration.
///
/// Creates a human-readable description of the tunnel forwarding,
/// properly handling IPv6 addresses with brackets.
///
/// # Examples
/// ```
/// use ssh_tunnel_common::{ForwardingConfig, ForwardingType, format_tunnel_description};
///
/// let config = ForwardingConfig {
///     forwarding_type: ForwardingType::Local,
///     bind_address: "127.0.0.1".to_string(),
///     local_port: Some(8080),
///     remote_host: Some("example.com".to_string()),
///     remote_port: Some(80),
/// };
///
/// assert_eq!(
///     format_tunnel_description(&config),
///     "local: 127.0.0.1:8080 → remote: example.com:80"
/// );
/// ```
pub fn format_tunnel_description(forwarding: &ForwardingConfig) -> String {
    match forwarding.forwarding_type {
        ForwardingType::Local => {
            let bind_address = &forwarding.bind_address;
            let local_port = forwarding.local_port.unwrap_or(0);
            let remote_host = forwarding.remote_host.as_deref().unwrap_or("localhost");
            let remote_port = forwarding.remote_port.unwrap_or(0);
            format!(
                "local: {} → remote: {}",
                format_host_port(bind_address, local_port),
                format_host_port(remote_host, remote_port)
            )
        }
        ForwardingType::Remote => {
            let bind_address = &forwarding.bind_address;
            let local_port = forwarding.local_port.unwrap_or(0);
            let remote_host = forwarding.remote_host.as_deref().unwrap_or("localhost");
            let remote_port = forwarding.remote_port.unwrap_or(0);
            format!(
                "remote: {} → local: {}",
                format_host_port(remote_host, remote_port),
                format_host_port(bind_address, local_port)
            )
        }
        ForwardingType::Dynamic => {
            let bind_address = &forwarding.bind_address;
            let local_port = forwarding.local_port.unwrap_or(1080);
            format!("SOCKS: {}", format_host_port(bind_address, local_port))
        }
    }
}
