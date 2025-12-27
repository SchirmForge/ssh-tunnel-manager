// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Event handling traits and types

use ssh_tunnel_common::{TunnelStatus, AuthRequest};
use uuid::Uuid;

/// Framework-agnostic event handler trait
///
/// GUI implementations (GTK, Qt) should implement this trait to handle
/// events from the daemon and update their UI accordingly.
pub trait TunnelEventHandler: Send + Sync {
    /// Called when tunnel status changes
    fn on_status_changed(&self, profile_id: Uuid, status: TunnelStatus);

    /// Called when authentication is required
    fn on_auth_required(&self, request: AuthRequest);

    /// Called when daemon connection state changes
    fn on_daemon_connected(&self, connected: bool);

    /// Called when an error occurs
    fn on_error(&self, profile_id: Option<Uuid>, error: String);

    /// Called when daemon info needs to be refreshed
    fn on_daemon_info_changed(&self);
}

/// GUI events that can be triggered
#[derive(Debug, Clone)]
pub enum GuiEvent {
    /// Profile list needs refresh
    ProfileListRefresh,

    /// Navigate to a specific page
    NavigateToProfile(Uuid),

    /// Show error message
    ShowError(String),

    /// Show success message
    ShowSuccess(String),
}
