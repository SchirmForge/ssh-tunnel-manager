// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// SSH Tunnel Manager - Common Library
// Shared types, utilities, and configuration structures

pub mod config;
pub mod daemon_client;
pub mod error;
pub mod network;
pub mod profile_manager;
pub mod tls;
pub mod types;

pub use config::{ConnectionConfig, ForwardingConfig, Profile, TunnelOptions};
pub use daemon_client::{
    add_auth_header, create_daemon_client, start_tunnel_with_events, stop_tunnel,
    ConnectionMode, DaemonClientConfig, TunnelEvent as DaemonTunnelEvent,
    TunnelEventHandler, TunnelStatusResponse,
};
pub use error::{Error, Result};
pub use network::is_loopback_address;
pub use profile_manager::{
    delete_profile_by_id, delete_profile_by_name, load_all_profiles, load_profile,
    load_profile_by_id, load_profile_by_name, profile_exists_by_id, profile_exists_by_name,
    profiles_dir, save_profile,
};
pub use tls::{create_insecure_tls_config, create_pinned_tls_config};
pub use types::{
    AuthRequest, AuthRequestType, AuthResponse, AuthType, ForwardingType, StartTunnelResult,
    TunnelEvent, TunnelStatus,
};

// Re-export commonly used external types
pub use chrono::{DateTime, Utc};
pub use uuid::Uuid;
