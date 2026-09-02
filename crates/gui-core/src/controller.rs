// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Central toolkit-independent controller, reducer, and immutable snapshots.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use ssh_tunnel_common::{
    AuthRequest, ConnectionMode, DaemonInfo, DateTime, Profile, TunnelStatus, Utc,
};
use uuid::Uuid;

use crate::actions::{
    ActionAvailability, AppCommand, ControllerEffect, ProfileAction, ProfileOperation,
};
use crate::auth::{AuthAnswerError, AuthPromptSnapshot, AuthQueue, AuthSubmission};
use crate::preferences::{SortMode, UiPreferences};
use crate::view_models::{ProfileDetailsViewModel, ProfileViewModel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureState {
    Available,
    UiOnlyWip { reason: String },
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureAvailability {
    pub auto_reconnect_runtime: FeatureState,
    pub compression_runtime: FeatureState,
    pub configurable_keepalive: FeatureState,
    pub tcp_keepalive: FeatureState,
    pub packet_window_tuning: FeatureState,
    pub stored_totp: FeatureState,
    pub daemon_shutdown: FeatureState,
    pub daemon_start: FeatureState,
    pub daemon_restart: FeatureState,
    pub ssh_config_import: FeatureState,
    pub remote_forwarding: FeatureState,
    pub dynamic_forwarding: FeatureState,
    pub connection_telemetry: FeatureState,
    pub tray: FeatureState,
}

impl Default for FeatureAvailability {
    fn default() -> Self {
        Self {
            auto_reconnect_runtime: FeatureState::UiOnlyWip {
                reason: "Profile settings are stored, but daemon enforcement is not complete"
                    .to_string(),
            },
            compression_runtime: FeatureState::UiOnlyWip {
                reason: "The current SSH build deliberately omits compression support".to_string(),
            },
            configurable_keepalive: FeatureState::UiOnlyWip {
                reason:
                    "The profile value is stored, but the daemon currently uses a fixed interval"
                        .to_string(),
            },
            tcp_keepalive: FeatureState::UiOnlyWip {
                reason: "The profile value is stored, but daemon enforcement is not implemented"
                    .to_string(),
            },
            packet_window_tuning: FeatureState::Available,
            stored_totp: FeatureState::UiOnlyWip {
                reason: "Secure TOTP storage and generation are not implemented".to_string(),
            },
            daemon_shutdown: FeatureState::UiOnlyWip {
                reason: "Daemon shutdown remains visible but is deferred by the current GUI scope"
                    .to_string(),
            },
            daemon_start: FeatureState::UiOnlyWip {
                reason: "The daemon does not expose a safe start operation".to_string(),
            },
            daemon_restart: FeatureState::UiOnlyWip {
                reason: "The daemon does not expose a safe restart operation".to_string(),
            },
            ssh_config_import: FeatureState::UiOnlyWip {
                reason: "SSH configuration import is not implemented".to_string(),
            },
            remote_forwarding: FeatureState::Unavailable {
                reason: "The daemon rejects remote forwarding".to_string(),
            },
            dynamic_forwarding: FeatureState::Unavailable {
                reason: "The daemon rejects dynamic/SOCKS forwarding".to_string(),
            },
            connection_telemetry: FeatureState::Unavailable {
                reason: "The daemon does not provide per-tunnel telemetry".to_string(),
            },
            tray: FeatureState::UiOnlyWip {
                reason: "Tray integration is deferred; shared actions are available".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiError {
    pub profile_id: Option<Uuid>,
    /// Display-only error detail. Reducer behavior is selected by event codes.
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ProfileSnapshot {
    pub view: ProfileViewModel,
    pub details: ProfileDetailsViewModel,
    pub pinned: bool,
    pub auto_reconnect: bool,
    pub pending_operation: Option<ProfileOperation>,
    pub actions: ActionAvailability,
}

/// One tunnel entry from the daemon's structured status inventory.
#[derive(Debug, Clone)]
pub struct TunnelRuntimeSnapshot {
    pub profile_id: Uuid,
    pub status: TunnelStatus,
    pub pending_auth: Option<AuthRequest>,
}

#[derive(Debug, Clone)]
pub struct AppSnapshot {
    pub revision: u64,
    /// Whether at least one complete health refresh has returned.
    pub has_refreshed: bool,
    pub refresh_in_flight: bool,
    pub daemon_connected: bool,
    pub daemon_connection_mode: ConnectionMode,
    pub daemon_info: Option<DaemonInfo>,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub profiles: Vec<ProfileSnapshot>,
    pub selected_profile: Option<Uuid>,
    pub active_auth: Option<AuthPromptSnapshot>,
    pub queued_auth_requests: usize,
    pub auth_submission_in_flight: bool,
    /// Error for the active authentication request, correlated by request ID.
    /// The text is display-only and must never select presentation behavior.
    pub active_auth_error: Option<String>,
    pub last_error: Option<UiError>,
    pub features: FeatureAvailability,
    pub preferences: UiPreferences,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationOutcome {
    Succeeded,
    Failed(String),
}

/// Structured inputs consumed by the reducer.
#[derive(Debug, Clone)]
pub enum ControllerEvent {
    RefreshFinished,
    ProfilesLoaded(Vec<Profile>),
    ProfileSaved(Box<Profile>),
    ProfileDeleted(Uuid),
    PreferencesLoaded(UiPreferences),
    DaemonConnectionMode(ConnectionMode),
    DaemonConnectionChanged(bool),
    /// Boxed like `ProfileSaved`: `DaemonInfo` is much larger than the other variants, and
    /// an unboxed copy would set the size of every `ControllerEvent`.
    DaemonInfoUpdated(Option<Box<DaemonInfo>>),
    HeartbeatReceived(DateTime<Utc>),
    TunnelInventory(Vec<TunnelRuntimeSnapshot>),
    TunnelStatusChanged {
        profile_id: Uuid,
        status: TunnelStatus,
    },
    AuthenticationRequired(AuthRequest),
    AuthenticationFinished {
        request_id: Uuid,
    },
    AuthenticationFinishedForTunnel {
        tunnel_id: Uuid,
    },
    AuthenticationSubmissionFailed {
        request_id: Uuid,
        message: String,
    },
    ProfileOperationFinished {
        profile_id: Uuid,
        operation: ProfileOperation,
        outcome: OperationOutcome,
    },
    ProtocolError {
        message: String,
    },
    RuntimeError {
        profile_id: Option<Uuid>,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandRejected {
    DaemonUnavailable,
    UnknownProfile(Uuid),
    ActionDisabled {
        profile_id: Uuid,
        action: ProfileAction,
    },
    UnknownAuthenticationRequest(Uuid),
    AuthenticationRequestNotActive(Uuid),
    AuthenticationSubmissionInFlight(Uuid),
    InvalidAuthenticationAnswer(AuthAnswerError),
}

impl fmt::Display for CommandRejected {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DaemonUnavailable => formatter.write_str("the daemon is not connected"),
            Self::UnknownProfile(profile_id) => {
                write!(formatter, "unknown profile {profile_id}")
            }
            Self::ActionDisabled { profile_id, action } => {
                write!(
                    formatter,
                    "action {action:?} is disabled for profile {profile_id}"
                )
            }
            Self::UnknownAuthenticationRequest(request_id) => {
                write!(formatter, "unknown authentication request {request_id}")
            }
            Self::AuthenticationRequestNotActive(request_id) => {
                write!(
                    formatter,
                    "authentication request {request_id} is not active"
                )
            }
            Self::AuthenticationSubmissionInFlight(request_id) => {
                write!(
                    formatter,
                    "authentication request {request_id} already has a submission in flight"
                )
            }
            Self::InvalidAuthenticationAnswer(error) => error.fmt(formatter),
        }
    }
}

impl Error for CommandRejected {}

/// Pure coordinator for state, commands, and effects.
///
/// I/O adapters execute [`ControllerEffect`] values and return structured
/// [`ControllerEvent`] values. No daemon-originating display string is inspected
/// by this controller to select behavior.
#[derive(Debug)]
pub struct AppController {
    profiles: Vec<Profile>,
    tunnel_statuses: HashMap<Uuid, TunnelStatus>,
    has_refreshed: bool,
    refresh_in_flight: bool,
    daemon_connected: bool,
    daemon_connection_mode: ConnectionMode,
    daemon_info: Option<DaemonInfo>,
    last_heartbeat: Option<DateTime<Utc>>,
    selected_profile: Option<Uuid>,
    pending_operations: HashMap<Uuid, ProfileOperation>,
    auth_queue: AuthQueue,
    auth_submission_in_flight: Option<Uuid>,
    auth_submission_error: Option<(Uuid, String)>,
    preferences: UiPreferences,
    features: FeatureAvailability,
    last_error: Option<UiError>,
    revision: u64,
}

impl Default for AppController {
    fn default() -> Self {
        Self::new()
    }
}

impl AppController {
    pub fn new() -> Self {
        Self::with_preferences(UiPreferences::default())
    }

    pub fn with_preferences(preferences: UiPreferences) -> Self {
        Self {
            profiles: Vec::new(),
            tunnel_statuses: HashMap::new(),
            has_refreshed: false,
            refresh_in_flight: false,
            daemon_connected: false,
            daemon_connection_mode: ConnectionMode::UnixSocket,
            daemon_info: None,
            last_heartbeat: None,
            selected_profile: None,
            pending_operations: HashMap::new(),
            auth_queue: AuthQueue::new(),
            auth_submission_in_flight: None,
            auth_submission_error: None,
            preferences,
            features: FeatureAvailability::default(),
            last_error: None,
            revision: 0,
        }
    }

    pub fn preferences(&self) -> &UiPreferences {
        &self.preferences
    }

    pub fn snapshot(&self) -> AppSnapshot {
        let profiles = self.ordered_profiles();
        let profiles = profiles
            .into_iter()
            .filter_map(|profile| {
                let status = self
                    .tunnel_statuses
                    .get(&profile.metadata.id)
                    .cloned()
                    .unwrap_or(TunnelStatus::NotConnected);
                if self.preferences.connected_only && !matches!(status, TunnelStatus::Connected) {
                    return None;
                }

                let pending_operation = self.pending_operations.get(&profile.metadata.id).copied();
                Some(ProfileSnapshot {
                    view: ProfileViewModel::from_profile(profile, status.clone()),
                    details: ProfileDetailsViewModel::from_profile(
                        profile,
                        self.daemon_connection_mode == ConnectionMode::UnixSocket,
                    ),
                    pinned: self.preferences.is_pinned(profile.metadata.id),
                    auto_reconnect: profile.options.auto_reconnect,
                    pending_operation,
                    actions: ActionAvailability::for_profile(
                        self.daemon_connected,
                        &status,
                        pending_operation,
                    ),
                })
            })
            .collect();

        AppSnapshot {
            revision: self.revision,
            has_refreshed: self.has_refreshed,
            refresh_in_flight: self.refresh_in_flight,
            daemon_connected: self.daemon_connected,
            daemon_connection_mode: self.daemon_connection_mode.clone(),
            daemon_info: self.daemon_info.clone(),
            last_heartbeat: self.last_heartbeat,
            profiles,
            selected_profile: self.selected_profile,
            active_auth: self.auth_queue.active().map(|request| {
                let mut prompt = AuthPromptSnapshot::from(request);
                prompt.profile_name = self
                    .profile(request.tunnel_id)
                    .map(|profile| profile.metadata.name.clone());
                prompt
            }),
            queued_auth_requests: self.auth_queue.pending_len(),
            auth_submission_in_flight: self.auth_submission_in_flight.is_some(),
            active_auth_error: self.auth_queue.active().and_then(|request| {
                self.auth_submission_error
                    .as_ref()
                    .filter(|(request_id, _)| *request_id == request.id)
                    .map(|(_, message)| message.clone())
            }),
            last_error: self.last_error.clone(),
            features: self.features.clone(),
            preferences: self.preferences.clone(),
        }
    }

    pub fn dispatch(
        &mut self,
        command: AppCommand,
    ) -> Result<Vec<ControllerEffect>, CommandRejected> {
        match command {
            AppCommand::Refresh => {
                if self.refresh_in_flight {
                    return Ok(Vec::new());
                }
                self.refresh_in_flight = true;
                self.last_error = None;
                self.bump_revision();
                Ok(vec![ControllerEffect::Refresh])
            }
            AppCommand::ShutdownDaemon => {
                if !self.daemon_connected {
                    return Err(CommandRejected::DaemonUnavailable);
                }
                Ok(vec![ControllerEffect::ShutdownDaemon])
            }
            AppCommand::SelectProfile(profile_id) => {
                if let Some(profile_id) = profile_id {
                    self.require_profile(profile_id)?;
                }
                self.selected_profile = profile_id;
                self.bump_revision();
                Ok(Vec::new())
            }
            AppCommand::CreateProfile => Ok(vec![ControllerEffect::OpenCreateProfile]),
            AppCommand::EditProfile(profile_id) => {
                self.require_action(profile_id, ProfileAction::Edit)?;
                Ok(vec![ControllerEffect::OpenEditProfile(profile_id)])
            }
            AppCommand::SaveProfile { request } => {
                let profile_id = request.profile.metadata.id;
                self.pending_operations
                    .insert(profile_id, ProfileOperation::Save);
                self.bump_revision();
                Ok(vec![ControllerEffect::SaveProfile { request }])
            }
            AppCommand::DuplicateProfile(profile_id) => {
                self.require_action(profile_id, ProfileAction::Duplicate)?;
                Ok(vec![ControllerEffect::OpenDuplicateProfile(profile_id)])
            }
            AppCommand::DeleteProfile(profile_id) => {
                self.require_action(profile_id, ProfileAction::Delete)?;
                Ok(vec![ControllerEffect::ConfirmDeleteProfile(profile_id)])
            }
            AppCommand::ConfirmDeleteProfile(profile_id) => self.queue_profile_effect(
                profile_id,
                ProfileAction::Delete,
                ProfileOperation::Delete,
                ControllerEffect::DeleteProfile(profile_id),
            ),
            AppCommand::ConnectProfile(profile_id) => self.queue_profile_effect(
                profile_id,
                ProfileAction::Connect,
                ProfileOperation::Connect,
                ControllerEffect::ConnectProfile(profile_id),
            ),
            AppCommand::CancelConnection(profile_id) => self.queue_profile_effect(
                profile_id,
                ProfileAction::CancelConnection,
                ProfileOperation::CancelConnection,
                ControllerEffect::CancelConnection(profile_id),
            ),
            AppCommand::DisconnectProfile(profile_id) => self.queue_profile_effect(
                profile_id,
                ProfileAction::Disconnect,
                ProfileOperation::Disconnect,
                ControllerEffect::DisconnectProfile(profile_id),
            ),
            AppCommand::RetryProfile(profile_id) => self.queue_profile_effect(
                profile_id,
                ProfileAction::Retry,
                ProfileOperation::Retry,
                ControllerEffect::RetryProfile(profile_id),
            ),
            AppCommand::SetProfilePinned { profile_id, pinned } => {
                self.require_action(profile_id, ProfileAction::Pin)?;
                if !self.preferences.set_pinned(profile_id, pinned) {
                    return Ok(Vec::new());
                }
                self.bump_revision();
                Ok(vec![ControllerEffect::PersistPreferences(
                    self.preferences.clone(),
                )])
            }
            AppCommand::MoveProfile {
                profile_id,
                new_index,
            } => {
                self.require_profile(profile_id)?;
                if !self.preferences.move_profile(profile_id, new_index) {
                    return Ok(Vec::new());
                }
                self.bump_revision();
                Ok(vec![ControllerEffect::PersistPreferences(
                    self.preferences.clone(),
                )])
            }
            AppCommand::SetConnectedOnly(connected_only) => {
                if !self.preferences.set_connected_only(connected_only) {
                    return Ok(Vec::new());
                }
                self.bump_revision();
                Ok(vec![ControllerEffect::PersistPreferences(
                    self.preferences.clone(),
                )])
            }
            AppCommand::SetSortMode(sort_mode) => {
                if !self.preferences.set_sort_mode(sort_mode) {
                    return Ok(Vec::new());
                }
                self.bump_revision();
                Ok(vec![ControllerEffect::PersistPreferences(
                    self.preferences.clone(),
                )])
            }
            AppCommand::SetAutoReconnect {
                profile_id,
                enabled,
            } => self.queue_profile_effect(
                profile_id,
                ProfileAction::ToggleAutoReconnect,
                ProfileOperation::ToggleAutoReconnect,
                ControllerEffect::SetAutoReconnect {
                    profile_id,
                    enabled,
                },
            ),
            AppCommand::AnswerAuthentication { request_id, answer } => {
                let active = self.active_auth_request(request_id)?.clone();
                if self.auth_submission_in_flight == Some(request_id) {
                    return Err(CommandRejected::AuthenticationSubmissionInFlight(
                        request_id,
                    ));
                }
                let submission = AuthSubmission::from_request(&active, answer)
                    .map_err(CommandRejected::InvalidAuthenticationAnswer)?;
                if self
                    .auth_submission_error
                    .as_ref()
                    .is_some_and(|(error_request_id, _)| *error_request_id == request_id)
                {
                    self.auth_submission_error = None;
                }
                self.auth_submission_in_flight = Some(request_id);
                self.bump_revision();
                Ok(vec![ControllerEffect::SubmitAuthentication(submission)])
            }
            AppCommand::CancelAuthentication { request_id } => {
                let active = self.active_auth_request(request_id)?;
                if self.auth_submission_in_flight == Some(request_id) {
                    return Err(CommandRejected::AuthenticationSubmissionInFlight(
                        request_id,
                    ));
                }
                let tunnel_id = active.tunnel_id;
                if self
                    .auth_submission_error
                    .as_ref()
                    .is_some_and(|(error_request_id, _)| *error_request_id == request_id)
                {
                    self.auth_submission_error = None;
                }
                self.auth_submission_in_flight = Some(request_id);
                self.bump_revision();
                Ok(vec![ControllerEffect::CancelAuthentication {
                    request_id,
                    tunnel_id,
                }])
            }
            AppCommand::ClearError => {
                if self.last_error.take().is_some() {
                    self.bump_revision();
                }
                Ok(Vec::new())
            }
        }
    }

    pub fn apply_event(&mut self, event: ControllerEvent) {
        match event {
            ControllerEvent::RefreshFinished => {
                self.has_refreshed = true;
                self.refresh_in_flight = false;
            }
            ControllerEvent::ProfilesLoaded(profiles) => {
                self.profiles = profiles;
                let known: std::collections::HashSet<Uuid> = self
                    .profiles
                    .iter()
                    .map(|profile| profile.metadata.id)
                    .collect();
                self.tunnel_statuses
                    .retain(|profile_id, _| known.contains(profile_id));
                self.pending_operations
                    .retain(|profile_id, _| known.contains(profile_id));
                if self
                    .selected_profile
                    .is_some_and(|profile_id| !known.contains(&profile_id))
                {
                    self.selected_profile = None;
                }
                self.reconcile_preferences();
            }
            ControllerEvent::ProfileSaved(profile) => {
                let profile = *profile;
                let profile_id = profile.metadata.id;
                if let Some(existing) = self
                    .profiles
                    .iter_mut()
                    .find(|existing| existing.metadata.id == profile_id)
                {
                    *existing = profile;
                } else {
                    self.profiles.push(profile);
                }
                self.pending_operations.remove(&profile_id);
                self.reconcile_preferences();
            }
            ControllerEvent::ProfileDeleted(profile_id) => {
                self.profiles
                    .retain(|profile| profile.metadata.id != profile_id);
                self.tunnel_statuses.remove(&profile_id);
                self.pending_operations.remove(&profile_id);
                let removed = self.auth_queue.remove_for_tunnel(profile_id);
                self.clear_auth_tracking_for_removed(&removed);
                self.preferences.remove_profile(profile_id);
                if self.selected_profile == Some(profile_id) {
                    self.selected_profile = None;
                }
            }
            ControllerEvent::PreferencesLoaded(preferences) => {
                self.preferences = preferences;
                self.reconcile_preferences();
            }
            ControllerEvent::DaemonConnectionMode(connection_mode) => {
                self.daemon_connection_mode = connection_mode;
            }
            ControllerEvent::DaemonConnectionChanged(connected) => {
                self.daemon_connected = connected;
                if !connected {
                    self.daemon_info = None;
                    self.last_heartbeat = None;
                    for status in self.tunnel_statuses.values_mut() {
                        *status = TunnelStatus::NotConnected;
                    }
                    self.pending_operations.clear();
                    self.auth_queue.clear();
                    self.auth_submission_in_flight = None;
                    self.auth_submission_error = None;
                }
            }
            ControllerEvent::DaemonInfoUpdated(info) => {
                self.daemon_info = info.map(|boxed| *boxed);
            }
            ControllerEvent::HeartbeatReceived(timestamp) => {
                self.daemon_connected = true;
                self.last_heartbeat = Some(timestamp);
            }
            ControllerEvent::TunnelInventory(tunnels) => {
                self.apply_tunnel_inventory(tunnels);
            }
            ControllerEvent::TunnelStatusChanged { profile_id, status } => {
                if self.profile(profile_id).is_none() {
                    self.last_error = Some(UiError {
                        profile_id: Some(profile_id),
                        message: "Received a status code for an unknown profile".to_string(),
                    });
                } else {
                    self.tunnel_statuses.insert(profile_id, status);
                    self.pending_operations.remove(&profile_id);
                }
            }
            ControllerEvent::AuthenticationRequired(request) => {
                // A new request ID for the same tunnel is structured proof that
                // the daemon consumed the submitted response and advanced to a
                // subsequent challenge. Do not inspect either prompt string.
                let completed_request_id = self.auth_submission_in_flight.and_then(|request_id| {
                    self.auth_queue
                        .active()
                        .filter(|active| {
                            active.id == request_id
                                && active.tunnel_id == request.tunnel_id
                                && active.id != request.id
                        })
                        .map(|active| active.id)
                });
                if let Some(request_id) = completed_request_id {
                    self.auth_queue.remove(request_id);
                    self.auth_submission_in_flight = None;
                    self.clear_auth_error(request_id);
                }
                self.auth_queue.enqueue(request);
            }
            ControllerEvent::AuthenticationFinished { request_id } => {
                self.auth_queue.remove(request_id);
                if self.auth_submission_in_flight == Some(request_id) {
                    self.auth_submission_in_flight = None;
                }
                self.clear_auth_error(request_id);
            }
            ControllerEvent::AuthenticationFinishedForTunnel { tunnel_id } => {
                let removed = self.auth_queue.remove_for_tunnel(tunnel_id);
                self.clear_auth_tracking_for_removed(&removed);
            }
            ControllerEvent::AuthenticationSubmissionFailed {
                request_id,
                message,
            } => {
                let profile_id = self
                    .auth_queue
                    .active()
                    .filter(|request| request.id == request_id)
                    .map(|request| request.tunnel_id);
                if self.auth_submission_in_flight == Some(request_id) {
                    self.auth_submission_in_flight = None;
                }
                self.auth_submission_error = Some((request_id, message.clone()));
                self.last_error = Some(UiError {
                    profile_id,
                    message,
                });
            }
            ControllerEvent::ProfileOperationFinished {
                profile_id,
                operation,
                outcome,
            } => {
                if self.pending_operations.get(&profile_id) == Some(&operation) {
                    self.pending_operations.remove(&profile_id);
                }
                if let OperationOutcome::Failed(message) = outcome {
                    self.last_error = Some(UiError {
                        profile_id: Some(profile_id),
                        message,
                    });
                }
            }
            ControllerEvent::ProtocolError { message } => {
                self.last_error = Some(UiError {
                    profile_id: None,
                    message,
                });
            }
            ControllerEvent::RuntimeError {
                profile_id,
                message,
            } => {
                self.last_error = Some(UiError {
                    profile_id,
                    message,
                });
            }
        }
        self.bump_revision();
    }

    fn apply_tunnel_inventory(&mut self, tunnels: Vec<TunnelRuntimeSnapshot>) {
        let known_profiles: HashSet<Uuid> = self
            .profiles
            .iter()
            .map(|profile| profile.metadata.id)
            .collect();
        let mut valid_tunnels = Vec::with_capacity(tunnels.len());
        let mut reported_auth_ids = HashSet::new();

        for tunnel in tunnels {
            if !known_profiles.contains(&tunnel.profile_id) {
                self.last_error = Some(UiError {
                    profile_id: Some(tunnel.profile_id),
                    message: "The daemon reported an unknown profile ID".to_string(),
                });
                continue;
            }

            if let Some(request) = &tunnel.pending_auth {
                if request.tunnel_id != tunnel.profile_id {
                    self.last_error = Some(UiError {
                        profile_id: Some(tunnel.profile_id),
                        message: "The daemon returned contradictory authentication identifiers"
                            .to_string(),
                    });
                    continue;
                }
                if !reported_auth_ids.insert(request.id) {
                    self.last_error = Some(UiError {
                        profile_id: Some(tunnel.profile_id),
                        message: "The daemon reused an authentication request ID".to_string(),
                    });
                    continue;
                }
            }

            valid_tunnels.push(tunnel);
        }

        for profile in &self.profiles {
            self.tunnel_statuses
                .insert(profile.metadata.id, TunnelStatus::NotConnected);
        }

        for tunnel in valid_tunnels {
            self.tunnel_statuses
                .insert(tunnel.profile_id, tunnel.status);
            self.pending_operations.remove(&tunnel.profile_id);
            if let Some(request) = tunnel.pending_auth {
                self.auth_queue.enqueue(request);
            }
        }

        self.auth_queue.retain_request_ids(&reported_auth_ids);
        if self
            .auth_submission_in_flight
            .is_some_and(|request_id| !reported_auth_ids.contains(&request_id))
        {
            self.auth_submission_in_flight = None;
        }
        if self
            .auth_submission_error
            .as_ref()
            .is_some_and(|(request_id, _)| !reported_auth_ids.contains(request_id))
        {
            self.auth_submission_error = None;
        }
    }

    fn clear_auth_error(&mut self, request_id: Uuid) {
        if self
            .auth_submission_error
            .as_ref()
            .is_some_and(|(error_request_id, _)| *error_request_id == request_id)
        {
            self.auth_submission_error = None;
        }
    }

    fn clear_auth_tracking_for_removed(&mut self, removed: &[AuthRequest]) {
        if removed
            .iter()
            .any(|request| self.auth_submission_in_flight == Some(request.id))
        {
            self.auth_submission_in_flight = None;
        }
        if let Some((request_id, _)) = self.auth_submission_error.as_ref() {
            if removed.iter().any(|request| request.id == *request_id) {
                self.auth_submission_error = None;
            }
        }
    }

    fn queue_profile_effect(
        &mut self,
        profile_id: Uuid,
        action: ProfileAction,
        operation: ProfileOperation,
        effect: ControllerEffect,
    ) -> Result<Vec<ControllerEffect>, CommandRejected> {
        self.require_action(profile_id, action)?;
        self.pending_operations.insert(profile_id, operation);
        self.bump_revision();
        Ok(vec![effect])
    }

    fn active_auth_request(&self, request_id: Uuid) -> Result<&AuthRequest, CommandRejected> {
        let Some(active) = self.auth_queue.active() else {
            return Err(CommandRejected::UnknownAuthenticationRequest(request_id));
        };
        if active.id == request_id {
            Ok(active)
        } else if self.auth_queue.contains(request_id) {
            Err(CommandRejected::AuthenticationRequestNotActive(request_id))
        } else {
            Err(CommandRejected::UnknownAuthenticationRequest(request_id))
        }
    }

    fn require_profile(&self, profile_id: Uuid) -> Result<&Profile, CommandRejected> {
        self.profile(profile_id)
            .ok_or(CommandRejected::UnknownProfile(profile_id))
    }

    fn require_action(
        &self,
        profile_id: Uuid,
        action: ProfileAction,
    ) -> Result<(), CommandRejected> {
        self.require_profile(profile_id)?;
        let status = self
            .tunnel_statuses
            .get(&profile_id)
            .cloned()
            .unwrap_or(TunnelStatus::NotConnected);
        let availability = ActionAvailability::for_profile(
            self.daemon_connected,
            &status,
            self.pending_operations.get(&profile_id).copied(),
        );
        if availability.is_enabled(action) {
            Ok(())
        } else {
            Err(CommandRejected::ActionDisabled { profile_id, action })
        }
    }

    fn profile(&self, profile_id: Uuid) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|profile| profile.metadata.id == profile_id)
    }

    fn reconcile_preferences(&mut self) {
        self.preferences
            .reconcile_profiles(self.profiles.iter().map(|profile| profile.metadata.id));
    }

    fn ordered_profiles(&self) -> Vec<&Profile> {
        match self.preferences.sort_mode {
            SortMode::Manual => {
                let mut profiles = Vec::with_capacity(self.profiles.len());
                for profile_id in self.preferences.display_order() {
                    if let Some(profile) = self.profile(profile_id) {
                        profiles.push(profile);
                    }
                }
                for profile in &self.profiles {
                    if !profiles
                        .iter()
                        .any(|known| known.metadata.id == profile.metadata.id)
                    {
                        profiles.push(profile);
                    }
                }
                profiles
            }
            SortMode::Name => {
                let mut profiles: Vec<&Profile> = self.profiles.iter().collect();
                profiles.sort_by(|left, right| {
                    let left_pinned = self.preferences.is_pinned(left.metadata.id);
                    let right_pinned = self.preferences.is_pinned(right.metadata.id);
                    right_pinned.cmp(&left_pinned).then_with(|| {
                        left.metadata
                            .name
                            .to_lowercase()
                            .cmp(&right.metadata.name.to_lowercase())
                    })
                });
                profiles
            }
        }
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ssh_tunnel_common::{
        AuthRequestType, AuthType, ConnectionConfig, ForwardingConfig, ForwardingType,
        PasswordStorage,
    };

    use super::*;
    use crate::auth::{AuthAnswer, AuthInputMode, AuthPromptKind};

    fn profile(name: &str) -> Profile {
        Profile::new(
            name.to_string(),
            ConnectionConfig {
                host: "example.test".to_string(),
                port: 22,
                user: "user".to_string(),
                auth_type: AuthType::Key,
                key_path: Some(PathBuf::from("/tmp/test-key")),
                password_storage: PasswordStorage::None,
            },
            ForwardingConfig {
                forwarding_type: ForwardingType::Local,
                local_port: Some(8080),
                remote_host: Some("localhost".to_string()),
                remote_port: Some(80),
                bind_address: "127.0.0.1".to_string(),
            },
        )
    }

    fn auth_request(
        tunnel_id: Uuid,
        auth_type: AuthRequestType,
        prompt: &str,
        hidden: bool,
    ) -> AuthRequest {
        AuthRequest {
            id: Uuid::new_v4(),
            tunnel_id,
            auth_type,
            prompt: prompt.to_string(),
            hidden,
        }
    }

    #[test]
    fn daemon_profile_actions_are_shared_and_code_driven() {
        let profile = profile("Primary");
        let profile_id = profile.metadata.id;
        let mut controller = AppController::new();
        controller.apply_event(ControllerEvent::ProfilesLoaded(vec![profile]));

        assert!(matches!(
            controller.dispatch(AppCommand::ConnectProfile(profile_id)),
            Err(CommandRejected::ActionDisabled {
                action: ProfileAction::Connect,
                ..
            })
        ));

        controller.apply_event(ControllerEvent::DaemonConnectionChanged(true));
        let effects = controller
            .dispatch(AppCommand::ConnectProfile(profile_id))
            .expect("connect should be enabled");
        assert!(matches!(
            effects.as_slice(),
            [ControllerEffect::ConnectProfile(id)] if *id == profile_id
        ));
        assert!(controller.snapshot().profiles[0].actions.cancel_connection);

        controller.apply_event(ControllerEvent::TunnelStatusChanged {
            profile_id,
            status: TunnelStatus::Connected,
        });
        let snapshot = controller.snapshot();
        assert!(snapshot.profiles[0].actions.disconnect);
        assert!(!snapshot.profiles[0].actions.connect);
    }

    #[test]
    fn duplicate_and_delete_use_editor_and_confirmation_effects_before_mutation() {
        let profile = profile("Work");
        let profile_id = profile.metadata.id;
        let mut controller = AppController::new();
        controller.apply_event(ControllerEvent::ProfilesLoaded(vec![profile]));

        let duplicate = controller
            .dispatch(AppCommand::DuplicateProfile(profile_id))
            .unwrap();
        assert!(matches!(
            duplicate.as_slice(),
            [ControllerEffect::OpenDuplicateProfile(id)] if *id == profile_id
        ));

        let request_delete = controller
            .dispatch(AppCommand::DeleteProfile(profile_id))
            .unwrap();
        assert!(matches!(
            request_delete.as_slice(),
            [ControllerEffect::ConfirmDeleteProfile(id)] if *id == profile_id
        ));
        assert!(
            controller
                .snapshot()
                .profiles
                .iter()
                .find(|profile| profile.view.id == profile_id)
                .unwrap()
                .actions
                .delete
        );

        let confirmed = controller
            .dispatch(AppCommand::ConfirmDeleteProfile(profile_id))
            .unwrap();
        assert!(matches!(
            confirmed.as_slice(),
            [ControllerEffect::DeleteProfile(id)] if *id == profile_id
        ));
        assert_eq!(
            controller
                .snapshot()
                .profiles
                .iter()
                .find(|profile| profile.view.id == profile_id)
                .unwrap()
                .pending_operation,
            Some(ProfileOperation::Delete)
        );
    }

    #[test]
    fn pinning_changes_snapshot_order_and_requests_persistence() {
        let first = profile("First");
        let second = profile("Second");
        let first_id = first.metadata.id;
        let second_id = second.metadata.id;
        let mut controller = AppController::new();
        controller.apply_event(ControllerEvent::ProfilesLoaded(vec![first, second]));

        let effects = controller
            .dispatch(AppCommand::SetProfilePinned {
                profile_id: second_id,
                pinned: true,
            })
            .expect("pin should be enabled");
        assert!(matches!(
            effects.as_slice(),
            [ControllerEffect::PersistPreferences(_)]
        ));

        let snapshot = controller.snapshot();
        assert_eq!(snapshot.profiles[0].view.id, second_id);
        assert_eq!(snapshot.profiles[1].view.id, first_id);
        assert!(snapshot.profiles[0].pinned);
    }

    #[test]
    fn list_filter_and_sort_preferences_are_shared_and_persisted() {
        let zebra = profile("Zebra");
        let alpha = profile("alpha");
        let zebra_id = zebra.metadata.id;
        let alpha_id = alpha.metadata.id;
        let mut controller = AppController::new();
        controller.apply_event(ControllerEvent::ProfilesLoaded(vec![zebra, alpha]));

        let effects = controller
            .dispatch(AppCommand::SetSortMode(SortMode::Name))
            .expect("name sorting should be accepted");
        assert!(matches!(
            effects.as_slice(),
            [ControllerEffect::PersistPreferences(preferences)]
                if preferences.sort_mode == SortMode::Name
        ));
        assert_eq!(controller.snapshot().profiles[0].view.id, alpha_id);

        let effects = controller
            .dispatch(AppCommand::SetConnectedOnly(true))
            .expect("connected-only filtering should be accepted");
        assert!(matches!(
            effects.as_slice(),
            [ControllerEffect::PersistPreferences(preferences)]
                if preferences.connected_only
        ));
        assert!(controller.snapshot().profiles.is_empty());

        controller.apply_event(ControllerEvent::TunnelStatusChanged {
            profile_id: zebra_id,
            status: TunnelStatus::Connected,
        });
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.profiles.len(), 1);
        assert_eq!(snapshot.profiles[0].view.id, zebra_id);

        assert!(controller
            .dispatch(AppCommand::SetConnectedOnly(true))
            .expect("idempotent filter command should be accepted")
            .is_empty());
    }

    #[test]
    fn auth_ui_and_answer_validation_ignore_misleading_prompt_text() {
        let profile = profile("Auth");
        let profile_id = profile.metadata.id;
        let mut controller = AppController::new();
        controller.apply_event(ControllerEvent::ProfilesLoaded(vec![profile]));
        let request = auth_request(
            profile_id,
            AuthRequestType::HostKeyVerification,
            "Please enter password to continue",
            true,
        );
        let request_id = request.id;
        controller.apply_event(ControllerEvent::AuthenticationRequired(request));

        let auth = controller
            .snapshot()
            .active_auth
            .expect("request should be active");
        assert_eq!(auth.kind, AuthPromptKind::HostKeyVerification);
        assert_eq!(auth.input_mode, AuthInputMode::HostKeyDecision);

        assert!(matches!(
            controller.dispatch(AppCommand::AnswerAuthentication {
                request_id,
                answer: AuthAnswer::Input("yes".to_string()),
            }),
            Err(CommandRejected::InvalidAuthenticationAnswer(
                AuthAnswerError::HostKeyDecisionRequired
            ))
        ));

        let effects = controller
            .dispatch(AppCommand::AnswerAuthentication {
                request_id,
                answer: AuthAnswer::HostKeyDecision(true),
            })
            .expect("typed host-key answer should be valid");
        assert!(matches!(
            effects.as_slice(),
            [ControllerEffect::SubmitAuthentication(submission)]
                if submission.request_id == request_id && submission.response == "yes"
        ));
    }

    #[test]
    fn authentication_requests_advance_fifo_after_structured_completion() {
        let profile = profile("Auth queue");
        let profile_id = profile.metadata.id;
        let mut controller = AppController::new();
        controller.apply_event(ControllerEvent::ProfilesLoaded(vec![profile]));
        let first = auth_request(profile_id, AuthRequestType::Password, "second factor", true);
        let second = auth_request(
            profile_id,
            AuthRequestType::TwoFactorCode,
            "password",
            false,
        );
        let first_id = first.id;
        let second_id = second.id;

        controller.apply_event(ControllerEvent::AuthenticationRequired(first));
        controller.apply_event(ControllerEvent::AuthenticationRequired(second));
        assert_eq!(
            controller.snapshot().active_auth.unwrap().request_id,
            first_id
        );
        assert_eq!(controller.snapshot().queued_auth_requests, 1);

        controller.apply_event(ControllerEvent::AuthenticationFinished {
            request_id: first_id,
        });
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.active_auth.unwrap().request_id, second_id);
        assert_eq!(snapshot.queued_auth_requests, 0);
    }

    #[test]
    fn new_structured_challenge_advances_submitted_request_for_same_tunnel() {
        let profile = profile("Sequential auth");
        let profile_id = profile.metadata.id;
        let first = auth_request(
            profile_id,
            AuthRequestType::Password,
            "This says host key, but is display-only",
            true,
        );
        let second = auth_request(
            profile_id,
            AuthRequestType::KeyboardInteractive,
            "This says password, but is display-only",
            false,
        );
        let first_id = first.id;
        let second_id = second.id;
        let mut controller = AppController::new();
        controller.apply_event(ControllerEvent::ProfilesLoaded(vec![profile]));
        controller.apply_event(ControllerEvent::AuthenticationRequired(first));
        controller
            .dispatch(AppCommand::AnswerAuthentication {
                request_id: first_id,
                answer: AuthAnswer::Input("secret".to_string()),
            })
            .expect("the first structured request should be submitted");

        controller.apply_event(ControllerEvent::AuthenticationRequired(second));

        let snapshot = controller.snapshot();
        let active = snapshot
            .active_auth
            .expect("the next challenge should be active");
        assert_eq!(active.request_id, second_id);
        assert_eq!(active.kind, AuthPromptKind::KeyboardInteractive);
        assert!(!snapshot.auth_submission_in_flight);
        assert_eq!(snapshot.queued_auth_requests, 0);
    }

    #[test]
    fn new_request_for_another_tunnel_does_not_complete_in_flight_auth() {
        let first_profile = profile("First auth");
        let second_profile = profile("Second auth");
        let first_profile_id = first_profile.metadata.id;
        let second_profile_id = second_profile.metadata.id;
        let first = auth_request(first_profile_id, AuthRequestType::Password, "first", true);
        let second = auth_request(
            second_profile_id,
            AuthRequestType::TwoFactorCode,
            "second",
            false,
        );
        let first_id = first.id;
        let mut controller = AppController::new();
        controller.apply_event(ControllerEvent::ProfilesLoaded(vec![
            first_profile,
            second_profile,
        ]));
        controller.apply_event(ControllerEvent::AuthenticationRequired(first));
        controller
            .dispatch(AppCommand::AnswerAuthentication {
                request_id: first_id,
                answer: AuthAnswer::Input("secret".to_string()),
            })
            .expect("the first request should be submitted");

        controller.apply_event(ControllerEvent::AuthenticationRequired(second));

        let snapshot = controller.snapshot();
        assert_eq!(snapshot.active_auth.unwrap().request_id, first_id);
        assert!(snapshot.auth_submission_in_flight);
        assert_eq!(snapshot.queued_auth_requests, 1);
    }

    #[test]
    fn tunnel_inventory_reconciles_status_and_auth_by_structured_ids() {
        let first = profile("First");
        let second = profile("Second");
        let first_id = first.metadata.id;
        let second_id = second.metadata.id;
        let request = auth_request(
            first_id,
            AuthRequestType::Password,
            "This text says connected",
            true,
        );
        let request_id = request.id;
        let mut controller = AppController::new();
        controller.apply_event(ControllerEvent::ProfilesLoaded(vec![first, second]));
        controller.apply_event(ControllerEvent::DaemonConnectionChanged(true));
        controller.apply_event(ControllerEvent::TunnelInventory(vec![
            TunnelRuntimeSnapshot {
                profile_id: first_id,
                status: TunnelStatus::WaitingForAuth,
                pending_auth: Some(request),
            },
        ]));

        let snapshot = controller.snapshot();
        let first_snapshot = snapshot
            .profiles
            .iter()
            .find(|profile| profile.view.id == first_id)
            .expect("first profile should exist");
        let second_snapshot = snapshot
            .profiles
            .iter()
            .find(|profile| profile.view.id == second_id)
            .expect("second profile should exist");
        assert_eq!(first_snapshot.view.status, TunnelStatus::WaitingForAuth);
        assert_eq!(second_snapshot.view.status, TunnelStatus::NotConnected);
        let active = snapshot.active_auth.expect("auth request should be active");
        assert_eq!(active.request_id, request_id);
        assert_eq!(active.profile_name.as_deref(), Some("First"));

        controller.apply_event(ControllerEvent::TunnelInventory(Vec::new()));
        let snapshot = controller.snapshot();
        assert!(snapshot.active_auth.is_none());
        assert!(snapshot
            .profiles
            .iter()
            .all(|profile| profile.view.status == TunnelStatus::NotConnected));
    }

    #[test]
    fn rejected_auth_submission_keeps_prompt_available_for_retry() {
        let profile = profile("Auth retry");
        let profile_id = profile.metadata.id;
        let request = auth_request(
            profile_id,
            AuthRequestType::Password,
            "accept host key",
            true,
        );
        let request_id = request.id;
        let mut controller = AppController::new();
        controller.apply_event(ControllerEvent::ProfilesLoaded(vec![profile]));
        controller.apply_event(ControllerEvent::AuthenticationRequired(request));
        controller
            .dispatch(AppCommand::AnswerAuthentication {
                request_id,
                answer: AuthAnswer::Input("secret".to_string()),
            })
            .expect("the active request should accept a typed text answer");
        assert!(controller.snapshot().auth_submission_in_flight);

        controller.apply_event(ControllerEvent::AuthenticationSubmissionFailed {
            request_id,
            message: "connected".to_string(),
        });
        let snapshot = controller.snapshot();
        assert!(!snapshot.auth_submission_in_flight);
        assert_eq!(snapshot.active_auth.unwrap().request_id, request_id);
        assert_eq!(snapshot.active_auth_error.as_deref(), Some("connected"));
        assert_eq!(snapshot.last_error.unwrap().profile_id, Some(profile_id));
    }

    #[test]
    fn authentication_cancel_is_correlated_and_retries_after_delivery_failure() {
        let profile = profile("Auth cancel");
        let profile_id = profile.metadata.id;
        let request = auth_request(
            profile_id,
            AuthRequestType::KeyboardInteractive,
            "This says connected, but remains display-only",
            false,
        );
        let request_id = request.id;
        let mut controller = AppController::new();
        controller.apply_event(ControllerEvent::ProfilesLoaded(vec![profile]));
        controller.apply_event(ControllerEvent::AuthenticationRequired(request));

        let effects = controller
            .dispatch(AppCommand::CancelAuthentication { request_id })
            .expect("the active request should be cancellable");
        assert!(matches!(
            effects.as_slice(),
            [ControllerEffect::CancelAuthentication {
                request_id: effect_request_id,
                tunnel_id,
            }] if *effect_request_id == request_id && *tunnel_id == profile_id
        ));
        assert!(controller.snapshot().auth_submission_in_flight);

        controller.apply_event(ControllerEvent::AuthenticationSubmissionFailed {
            request_id,
            message: "stop request could not be delivered".to_string(),
        });
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.active_auth.unwrap().request_id, request_id);
        assert!(!snapshot.auth_submission_in_flight);
        assert_eq!(
            snapshot.active_auth_error.as_deref(),
            Some("stop request could not be delivered")
        );
    }

    #[test]
    fn daemon_lifecycle_scope_and_feature_states_are_explicit() {
        let mut controller = AppController::new();
        assert!(matches!(
            controller.dispatch(AppCommand::ShutdownDaemon),
            Err(CommandRejected::DaemonUnavailable)
        ));

        let features = controller.snapshot().features;
        assert!(matches!(
            features.daemon_shutdown,
            FeatureState::UiOnlyWip { .. }
        ));
        assert_eq!(features.packet_window_tuning, FeatureState::Available);
        assert!(matches!(
            features.compression_runtime,
            FeatureState::UiOnlyWip { .. }
        ));
        assert!(matches!(
            features.dynamic_forwarding,
            FeatureState::Unavailable { .. }
        ));

        controller.apply_event(ControllerEvent::DaemonConnectionChanged(true));
        assert!(matches!(
            controller
                .dispatch(AppCommand::ShutdownDaemon)
                .expect("the reusable core command remains implemented")
                .as_slice(),
            [ControllerEffect::ShutdownDaemon]
        ));
    }

    #[test]
    fn refresh_state_is_structured_idempotent_and_explicitly_completed() {
        let mut controller = AppController::new();
        assert!(!controller.snapshot().has_refreshed);
        assert!(!controller.snapshot().refresh_in_flight);

        let effects = controller
            .dispatch(AppCommand::Refresh)
            .expect("refresh should be available during initial state");
        assert!(matches!(effects.as_slice(), [ControllerEffect::Refresh]));
        assert!(controller.snapshot().refresh_in_flight);
        assert!(controller
            .dispatch(AppCommand::Refresh)
            .expect("a repeated refresh should be harmless")
            .is_empty());

        controller.apply_event(ControllerEvent::RefreshFinished);
        let snapshot = controller.snapshot();
        assert!(snapshot.has_refreshed);
        assert!(!snapshot.refresh_in_flight);
    }

    #[test]
    fn unknown_status_code_target_fails_closed() {
        let mut controller = AppController::new();
        let unknown = Uuid::new_v4();
        controller.apply_event(ControllerEvent::TunnelStatusChanged {
            profile_id: unknown,
            status: TunnelStatus::Connected,
        });

        let snapshot = controller.snapshot();
        assert!(snapshot.profiles.is_empty());
        assert_eq!(snapshot.last_error.unwrap().profile_id, Some(unknown));
    }
}
