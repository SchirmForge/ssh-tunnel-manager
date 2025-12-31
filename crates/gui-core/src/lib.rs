// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Framework-agnostic GUI core for SSH Tunnel Manager
//!
//! This crate contains shared business logic, state management, and data models
//! that are used by both GTK and Qt GUI implementations.

pub mod state;
pub mod events;
pub mod profiles;
pub mod view_models;
pub mod daemon;

// Re-export commonly used types
pub use state::AppCore;
pub use events::{TunnelEventHandler, GuiEvent};
pub use profiles::{load_profiles, save_profile, delete_profile, validate_profile, profile_name_exists};
pub use view_models::{ProfileViewModel, StatusColor};
pub use daemon::{
    DaemonClient, EventListener, TunnelEvent,
    load_daemon_config, get_cli_config_path,
    config::{ConfigStatus, check_config_status, load_snippet_config, save_daemon_config, save_skip_ssh_warning_preference, daemon_config_snippet_exists},
};

// Re-export types from common crate for convenience
pub use ssh_tunnel_common::{
    Profile, TunnelStatus, AuthRequest, AuthType, ForwardingType,
    DaemonInfo, ConnectionMode,
};
