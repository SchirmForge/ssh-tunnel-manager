// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! View models - Data structures prepared for UI display

use ssh_tunnel_common::{Profile, TunnelStatus, AuthType};
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
    pub can_start: bool,
    pub can_stop: bool,
}

/// Status color for UI indicators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusColor {
    Green,   // Connected
    Orange,  // Transitional (Connecting, Reconnecting, WaitingForAuth)
    Red,     // Failed
    Gray,    // NotConnected, Disconnected
}

impl ProfileViewModel {
    /// Create view model from Profile and current status
    pub fn from_profile(profile: &Profile, status: TunnelStatus) -> Self {
        let status_color = Self::status_color_for(&status);
        let status_text = Self::status_text_for(&status);
        let can_start = matches!(status, TunnelStatus::NotConnected | TunnelStatus::Disconnected | TunnelStatus::Failed(_));
        let can_stop = !matches!(status, TunnelStatus::NotConnected | TunnelStatus::Disconnected);

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
            can_start,
            can_stop,
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
            TunnelStatus::NotConnected
            | TunnelStatus::Disconnected => StatusColor::Gray,
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
    statuses: &std::collections::HashMap<Uuid, TunnelStatus>
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
