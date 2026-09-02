// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! View models - Data structures prepared for UI display

use ssh_tunnel_common::{AuthType, ForwardingType, PasswordStorage, Profile, TunnelStatus};
use uuid::Uuid;

/// Profile data prepared for UI display
#[derive(Debug, Clone)]
pub struct ProfileViewModel {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub user: String,
    pub status: TunnelStatus,
    pub status_color: StatusColor,
    pub status_text: String,
    pub connection_summary: String,
    pub forwarding_description: String,
    pub auth_type_display: String,
}

/// Complete profile details prepared without exposing any credential value.
#[derive(Debug, Clone)]
pub struct ProfileDetailsViewModel {
    pub forwarding_type: ForwardingType,
    pub forwarding_type_display: &'static str,
    pub unsupported_forwarding: bool,
    pub bind_address: String,
    pub local_port: Option<u16>,
    pub remote_host: Option<String>,
    pub remote_port: Option<u16>,
    pub auth_type_display: &'static str,
    pub key_path: Option<String>,
    pub password_storage: PasswordStorage,
    pub password_storage_display: &'static str,
    pub compression: bool,
    pub keepalive_interval: u64,
    pub auto_reconnect: bool,
    pub reconnect_attempts: u32,
    pub reconnect_delay: u64,
    pub tcp_keepalive: bool,
    pub max_packet_size: u32,
    pub window_size: u32,
}

impl ProfileDetailsViewModel {
    pub fn from_profile(profile: &Profile, daemon_is_local: bool) -> Self {
        let forwarding_type_display = match profile.forwarding.forwarding_type {
            ForwardingType::Local => "Local",
            ForwardingType::Remote => "Remote",
            ForwardingType::Dynamic => "Dynamic / SOCKS",
        };
        let password_storage = profile
            .connection
            .password_storage
            .resolved(daemon_is_local);
        let password_storage_display = match password_storage {
            PasswordStorage::None => "Not stored",
            PasswordStorage::Client => "This client",
            PasswordStorage::DaemonHost => "Daemon host",
            PasswordStorage::File => "Daemon-host file · WIP",
            PasswordStorage::Keychain => unreachable!("legacy storage was resolved"),
        };

        Self {
            forwarding_type: profile.forwarding.forwarding_type.clone(),
            forwarding_type_display,
            unsupported_forwarding: !matches!(
                profile.forwarding.forwarding_type,
                ForwardingType::Local
            ),
            bind_address: profile.forwarding.bind_address.clone(),
            local_port: profile.forwarding.local_port,
            remote_host: profile.forwarding.remote_host.clone(),
            remote_port: profile.forwarding.remote_port,
            auth_type_display: match profile.connection.auth_type {
                AuthType::Key => "SSH key",
                AuthType::Password => "Password",
                AuthType::PasswordWith2FA => "Password + 2FA",
            },
            key_path: profile
                .connection
                .key_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            password_storage,
            password_storage_display,
            compression: profile.options.compression,
            keepalive_interval: profile.options.keepalive_interval,
            auto_reconnect: profile.options.auto_reconnect,
            reconnect_attempts: profile.options.reconnect_attempts,
            reconnect_delay: profile.options.reconnect_delay,
            tcp_keepalive: profile.options.tcp_keepalive,
            max_packet_size: profile.options.max_packet_size,
            window_size: profile.options.window_size,
        }
    }
}

/// Status color for UI indicators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusColor {
    Green,  // Connected
    Orange, // Transitional (Connecting, Reconnecting, WaitingForAuth)
    Red,    // Failed
    Gray,   // NotConnected, Disconnected
}

impl ProfileViewModel {
    /// Create view model from Profile and current status
    pub fn from_profile(profile: &Profile, status: TunnelStatus) -> Self {
        let status_color = Self::status_color_for(&status);
        let status_text = Self::status_text_for(&status);
        Self {
            id: profile.metadata.id,
            name: profile.metadata.name.clone(),
            host: profile.connection.host.clone(),
            user: profile.connection.user.clone(),
            status,
            status_color,
            status_text: status_text.to_string(),
            connection_summary: Self::format_connection_summary(profile),
            forwarding_description: Self::format_forwarding(profile),
            auth_type_display: Self::format_auth_type(profile),
        }
    }

    /// Get status color based on current status
    pub fn status_color_for(status: &TunnelStatus) -> StatusColor {
        match status {
            TunnelStatus::Connected => StatusColor::Green,
            TunnelStatus::Connecting
            | TunnelStatus::WaitingForAuth
            | TunnelStatus::Reconnecting
            | TunnelStatus::Disconnecting => StatusColor::Orange,
            TunnelStatus::Failed(_) => StatusColor::Red,
            TunnelStatus::NotConnected | TunnelStatus::Disconnected => StatusColor::Gray,
        }
    }

    /// Get human-readable status text
    pub fn status_text_for(status: &TunnelStatus) -> &'static str {
        match status {
            TunnelStatus::NotConnected => "Not Connected",
            TunnelStatus::Connecting => "Connecting...",
            TunnelStatus::Connected => "Connected",
            TunnelStatus::Disconnecting => "Disconnecting...",
            TunnelStatus::Disconnected => "Disconnected",
            TunnelStatus::Failed(_) => "Failed",
            TunnelStatus::WaitingForAuth => "Waiting for Auth",
            TunnelStatus::Reconnecting => "Reconnecting...",
        }
    }

    fn format_connection_summary(profile: &Profile) -> String {
        format!("{}@{}", profile.connection.user, profile.connection.host)
    }

    fn format_forwarding(profile: &Profile) -> String {
        use ssh_tunnel_common::format_tunnel_description;
        format_tunnel_description(&profile.forwarding)
    }

    fn format_auth_type(profile: &Profile) -> String {
        match &profile.connection.auth_type {
            AuthType::Key => {
                if let Some(ref key_path) = profile.connection.key_path {
                    format!("SSH Key: {}", key_path.display())
                } else {
                    "SSH Key (no path set)".to_string()
                }
            }
            AuthType::Password => "Password".to_string(),
            AuthType::PasswordWith2FA => "Password + 2FA".to_string(),
        }
    }
}

/// Create view models for all profiles with current statuses
pub fn create_profile_view_models(
    profiles: &[Profile],
    statuses: &std::collections::HashMap<Uuid, TunnelStatus>,
) -> Vec<ProfileViewModel> {
    profiles
        .iter()
        .map(|profile| {
            let status = statuses
                .get(&profile.metadata.id)
                .cloned()
                .unwrap_or(TunnelStatus::NotConnected);
            ProfileViewModel::from_profile(profile, status)
        })
        .collect()
}
