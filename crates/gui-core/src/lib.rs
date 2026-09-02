// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Framework-agnostic GUI core for SSH Tunnel Manager
//!
//! This crate contains shared business logic, state management, and data models
//! used by toolkit adapters such as the GTK GUI and a future system tray.

pub mod actions;
pub mod auth;
pub mod client_setup;
pub mod controller;
pub mod daemon;
pub mod editor;
pub mod preferences;
pub mod profiles;
pub mod runtime;
pub mod state;
pub mod view_models;

// Re-export commonly used types
pub use actions::{
    ActionAvailability, AppCommand, ControllerEffect, ProfileAction, ProfileOperation,
};
pub use auth::{
    AuthAnswer, AuthAnswerError, AuthInputMode, AuthPromptKind, AuthPromptSnapshot, AuthQueue,
    AuthSubmission,
};
pub use client_setup::{
    ClientSetupDiscovery, ClientSetupDraft, ClientSetupField, ClientSetupIssue,
    ClientSetupIssueKind, ClientSetupRepository, ClientSetupValidationCode,
    ClientSetupValidationError, ClientSetupValidationErrors,
};
pub use controller::{
    AppController, AppSnapshot, CommandRejected, ControllerEvent, FeatureAvailability,
    FeatureState, OperationOutcome, ProfileSnapshot, TunnelRuntimeSnapshot, UiError,
};
pub use daemon::{
    config::{
        check_config_status, daemon_config_snippet_exists, load_snippet_config, save_daemon_config,
        save_skip_ssh_warning_preference, ConfigStatus,
    },
    get_cli_config_path, load_daemon_config, DaemonClient, EventListener, TunnelEvent,
};
pub use editor::{
    CredentialUpdate, EditorField, EditorValidationError, EditorValidationErrors,
    ProfileDeletionRequest, ProfileEditorDraft, ProfileEditorMode, ProfileEditorSession,
    ProfileSaveRequest, SecretValue, StoredCredentialState,
};
pub use preferences::{
    SortMode, UiPreferences, UiPreferencesLoad, UiPreferencesRepository,
    CURRENT_UI_PREFERENCES_VERSION,
};
pub use profiles::{
    delete_profile, load_profiles, profile_name_exists, save_profile, validate_profile,
};
pub use runtime::{map_sse_event, AppRuntime, PresentationRequest, RuntimeResult};
pub use state::AppCore;
pub use view_models::{ProfileDetailsViewModel, ProfileViewModel, StatusColor};

// Re-export types from common crate for convenience
pub use ssh_tunnel_common::{
    AuthRequest, AuthType, ConnectionMode, DaemonClientConfig, DaemonInfo, ForwardingType,
    PasswordStorage, Profile, TunnelStatus,
};
