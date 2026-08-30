// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Framework-agnostic GUI core for SSH Tunnel Manager
//!
//! This crate contains shared business logic, state management, and data models
//! that are used by both GTK and Qt GUI implementations.

pub mod daemon;
pub mod events;
pub mod profiles;
pub mod state;
pub mod view_models;

// Re-export commonly used types
pub use daemon::{
    config::{
        check_config_status, daemon_config_snippet_exists, load_snippet_config, save_daemon_config,
        save_skip_ssh_warning_preference, ConfigStatus,
    },
    get_cli_config_path, load_daemon_config, DaemonClient, EventListener, TunnelEvent,
};
pub use events::{GuiEvent, TunnelEventHandler};
pub use profiles::{
    delete_profile, load_profiles, profile_name_exists, save_profile, validate_profile,
};
pub use state::AppCore;
pub use view_models::{ProfileViewModel, StatusColor};

// Re-export types from common crate for convenience
pub use ssh_tunnel_common::{
    AuthRequest, AuthType, ConnectionMode, DaemonInfo, ForwardingType, Profile, TunnelStatus,
};
