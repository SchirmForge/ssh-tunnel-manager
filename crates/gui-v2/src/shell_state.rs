// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Presentation state derived only from structured core fields.

use ssh_tunnel_gui_core::{AppSnapshot, FeatureState, TunnelStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonBadgeState {
    Checking,
    Online { active_tunnels: usize },
    Offline,
}

impl DaemonBadgeState {
    pub fn from_snapshot(snapshot: &AppSnapshot) -> Self {
        if !snapshot.has_refreshed {
            return Self::Checking;
        }
        Self::from_statuses(
            snapshot.daemon_connected,
            snapshot.profiles.iter().map(|profile| &profile.view.status),
        )
    }

    pub fn from_statuses<'a>(
        daemon_connected: bool,
        statuses: impl IntoIterator<Item = &'a TunnelStatus>,
    ) -> Self {
        if !daemon_connected {
            return Self::Offline;
        }

        let active_tunnels = statuses
            .into_iter()
            .filter(|status| matches!(status, TunnelStatus::Connected))
            .count();
        Self::Online { active_tunnels }
    }

    pub fn label(self) -> String {
        match self {
            Self::Checking => "Checking daemon…".to_string(),
            Self::Online { active_tunnels: 0 } => "Daemon running".to_string(),
            Self::Online { active_tunnels: 1 } => "Daemon running · 1 active".to_string(),
            Self::Online { active_tunnels } => {
                format!("Daemon running · {active_tunnels} active")
            }
            Self::Offline => "Daemon offline".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityPresentation {
    Available,
    Wip { reason: String },
    Unavailable { reason: String },
}

impl From<&FeatureState> for CapabilityPresentation {
    fn from(feature: &FeatureState) -> Self {
        match feature {
            FeatureState::Available => Self::Available,
            FeatureState::UiOnlyWip { reason } => Self::Wip {
                reason: reason.clone(),
            },
            FeatureState::Unavailable { reason } => Self::Unavailable {
                reason: reason.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellViewState {
    pub daemon: DaemonBadgeState,
    pub profile_count: usize,
    pub has_error: bool,
    pub error_message: Option<String>,
}

impl ShellViewState {
    pub fn from_snapshot(snapshot: &AppSnapshot) -> Self {
        Self {
            daemon: DaemonBadgeState::from_snapshot(snapshot),
            profile_count: snapshot.profiles.len(),
            has_error: snapshot.last_error.is_some(),
            error_message: snapshot
                .last_error
                .as_ref()
                .map(|error| error.message.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_status_text_cannot_inflate_the_active_tunnel_count() {
        let statuses = [
            TunnelStatus::Connected,
            TunnelStatus::Failed("Connected and running".to_string()),
            TunnelStatus::WaitingForAuth,
        ];

        assert_eq!(
            DaemonBadgeState::from_statuses(true, statuses.iter()),
            DaemonBadgeState::Online { active_tunnels: 1 }
        );
    }

    #[test]
    fn offline_code_wins_over_connected_statuses() {
        let statuses = [TunnelStatus::Connected, TunnelStatus::Connected];

        assert_eq!(
            DaemonBadgeState::from_statuses(false, statuses.iter()),
            DaemonBadgeState::Offline
        );
    }

    #[test]
    fn checking_is_an_explicit_state_not_inferred_from_display_text() {
        assert_eq!(DaemonBadgeState::Checking.label(), "Checking daemon…");
    }

    #[test]
    fn capability_presentation_is_selected_by_variant() {
        let feature = FeatureState::UiOnlyWip {
            reason: "Available and complete".to_string(),
        };

        assert!(matches!(
            CapabilityPresentation::from(&feature),
            CapabilityPresentation::Wip { .. }
        ));
    }
}
