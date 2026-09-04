// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Toolkit-independent commands and action availability.

use ssh_tunnel_common::TunnelStatus;
use uuid::Uuid;

use crate::auth::{AuthAnswer, AuthSubmission};
use crate::editor::ProfileSaveRequest;
use crate::preferences::{SortMode, UiPreferences};

/// A profile action that can be exposed by any presentation adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileAction {
    OpenDetails,
    Connect,
    CancelConnection,
    Disconnect,
    Reconnect,
    Retry,
    Edit,
    Duplicate,
    Delete,
    Pin,
    ToggleAutoReconnect,
}

/// An operation currently awaiting completion from an I/O adapter or daemon event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileOperation {
    Connect,
    CancelConnection,
    Disconnect,
    Reconnect,
    Retry,
    Save,
    Duplicate,
    Delete,
    ToggleAutoReconnect,
}

/// Enabled state for all profile actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionAvailability {
    pub open_details: bool,
    pub connect: bool,
    pub cancel_connection: bool,
    pub disconnect: bool,
    pub reconnect: bool,
    pub retry: bool,
    pub edit: bool,
    pub duplicate: bool,
    pub delete: bool,
    pub pin: bool,
    pub toggle_auto_reconnect: bool,
}

impl ActionAvailability {
    /// Derive action state exclusively from structured daemon and controller state.
    pub fn for_profile(
        daemon_connected: bool,
        status: &TunnelStatus,
        pending_operation: Option<ProfileOperation>,
    ) -> Self {
        let inactive = !profile_changes_require_reconnect(status);
        let connectable = matches!(
            status,
            TunnelStatus::NotConnected | TunnelStatus::Disconnected
        );
        let connecting = matches!(
            status,
            TunnelStatus::Connecting | TunnelStatus::WaitingForAuth | TunnelStatus::Reconnecting
        );
        let operation_idle = pending_operation.is_none();
        let connect_requested = pending_operation == Some(ProfileOperation::Connect);

        Self {
            open_details: true,
            connect: daemon_connected && operation_idle && connectable,
            cancel_connection: daemon_connected
                && pending_operation != Some(ProfileOperation::CancelConnection)
                && (connecting || connect_requested),
            disconnect: daemon_connected
                && operation_idle
                && matches!(status, TunnelStatus::Connected),
            reconnect: daemon_connected && operation_idle,
            retry: daemon_connected && operation_idle && matches!(status, TunnelStatus::Failed(_)),
            // Editing is independent of tunnel state. A successful save while
            // the old profile is active offers an explicit reconnect.
            edit: operation_idle,
            duplicate: operation_idle,
            delete: operation_idle && inactive,
            pin: true,
            toggle_auto_reconnect: operation_idle,
        }
    }

    pub fn is_enabled(&self, action: ProfileAction) -> bool {
        match action {
            ProfileAction::OpenDetails => self.open_details,
            ProfileAction::Connect => self.connect,
            ProfileAction::CancelConnection => self.cancel_connection,
            ProfileAction::Disconnect => self.disconnect,
            ProfileAction::Reconnect => self.reconnect,
            ProfileAction::Retry => self.retry,
            ProfileAction::Edit => self.edit,
            ProfileAction::Duplicate => self.duplicate,
            ProfileAction::Delete => self.delete,
            ProfileAction::Pin => self.pin,
            ProfileAction::ToggleAutoReconnect => self.toggle_auto_reconnect,
        }
    }
}

/// User intent accepted by the shared application controller.
#[derive(Debug, Clone)]
pub enum AppCommand {
    Refresh,
    ShutdownDaemon,
    SelectProfile(Option<Uuid>),
    CreateProfile,
    EditProfile(Uuid),
    SaveProfile {
        request: Box<ProfileSaveRequest>,
    },
    DuplicateProfile(Uuid),
    DeleteProfile(Uuid),
    ConfirmDeleteProfile(Uuid),
    ConnectProfile(Uuid),
    CancelConnection(Uuid),
    DisconnectProfile(Uuid),
    ReconnectProfile(Uuid),
    RetryProfile(Uuid),
    SetProfilePinned {
        profile_id: Uuid,
        pinned: bool,
    },
    MoveProfile {
        profile_id: Uuid,
        new_index: usize,
    },
    SetConnectedOnly(bool),
    SetSortMode(SortMode),
    SetAutoReconnect {
        profile_id: Uuid,
        enabled: bool,
    },
    AnswerAuthentication {
        request_id: Uuid,
        answer: AuthAnswer,
    },
    CancelAuthentication {
        request_id: Uuid,
    },
    ClearError,
}

/// Side effect requested by the pure controller.
///
/// GTK, a future tray, or another adapter executes these effects and feeds the
/// resulting structured events back into the controller.
#[derive(Debug, Clone)]
pub enum ControllerEffect {
    Refresh,
    ShutdownDaemon,
    OpenCreateProfile,
    OpenEditProfile(Uuid),
    OpenDuplicateProfile(Uuid),
    ConfirmDeleteProfile(Uuid),
    SaveProfile {
        request: Box<ProfileSaveRequest>,
        offer_reconnect: bool,
    },
    DeleteProfile(Uuid),
    ConnectProfile(Uuid),
    CancelConnection(Uuid),
    DisconnectProfile(Uuid),
    ReconnectProfile(Uuid),
    RetryProfile(Uuid),
    SetAutoReconnect {
        profile_id: Uuid,
        enabled: bool,
    },
    PersistPreferences(UiPreferences),
    SubmitAuthentication(AuthSubmission),
    CancelAuthentication {
        request_id: Uuid,
        tunnel_id: Uuid,
    },
}

/// Whether the daemon is using (or trying to use) the currently loaded profile.
///
/// This decision is deliberately based only on the structured tunnel status.
pub(crate) fn profile_changes_require_reconnect(status: &TunnelStatus) -> bool {
    matches!(
        status,
        TunnelStatus::Connecting
            | TunnelStatus::WaitingForAuth
            | TunnelStatus::Connected
            | TunnelStatus::Reconnecting
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_is_derived_from_codes_not_status_text() {
        let connected = ActionAvailability::for_profile(true, &TunnelStatus::Connected, None);
        assert!(connected.disconnect);
        assert!(!connected.connect);
        assert!(connected.edit);
        assert!(connected.reconnect);

        let failed = ActionAvailability::for_profile(
            true,
            &TunnelStatus::Failed("Connected; please enter password".to_string()),
            None,
        );
        assert!(!failed.connect);
        assert!(failed.retry);
        assert!(failed.edit);
        assert!(!failed.disconnect);
    }

    #[test]
    fn reconnect_notice_uses_only_active_status_codes() {
        for status in [
            TunnelStatus::Connecting,
            TunnelStatus::WaitingForAuth,
            TunnelStatus::Connected,
            TunnelStatus::Reconnecting,
        ] {
            assert!(profile_changes_require_reconnect(&status));
            assert!(ActionAvailability::for_profile(true, &status, None).edit);
        }

        assert!(!profile_changes_require_reconnect(&TunnelStatus::Failed(
            "Connected; reconnect now".to_string()
        )));
    }

    #[test]
    fn a_pending_connect_enables_only_the_structured_cancel_path() {
        let availability = ActionAvailability::for_profile(
            true,
            &TunnelStatus::NotConnected,
            Some(ProfileOperation::Connect),
        );

        assert!(!availability.connect);
        assert!(availability.cancel_connection);
        assert!(!availability.disconnect);
        assert!(!availability.delete);
    }

    #[test]
    fn daemon_actions_are_disabled_while_offline() {
        let availability =
            ActionAvailability::for_profile(false, &TunnelStatus::Disconnected, None);

        assert!(!availability.connect);
        assert!(!availability.cancel_connection);
        assert!(!availability.disconnect);
        assert!(availability.edit);
        assert!(availability.pin);
    }
}
