// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Shared state for the tray application

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ssh_tunnel_common::{DaemonClientConfig, Profile};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Connection status for daemon and tunnels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Daemon not reachable
    Disconnected,
    /// Daemon connected, no active tunnels
    Connected,
    /// Daemon connected with at least one active tunnel
    Active,
}

/// State of a tunnel connection
#[derive(Debug, Clone)]
pub struct TunnelState {
    pub profile_id: Uuid,
    pub profile_name: String,
    pub connected_at: DateTime<Utc>,
}

/// Recent profile usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentProfile {
    pub profile_id: Uuid,
    pub profile_name: String,
    pub last_used: DateTime<Utc>,
}

/// Persistent state stored on disk
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistentState {
    pub recent_profiles: Vec<RecentProfile>,
}

/// Shared application state
pub struct TrayState {
    /// Current connection status
    pub status: ConnectionStatus,

    /// Active tunnel connections
    pub active_tunnels: HashMap<Uuid, TunnelState>,

    /// Last heartbeat received
    pub last_heartbeat: Option<DateTime<Utc>>,

    /// Daemon client configuration
    pub daemon_config: DaemonClientConfig,

    /// Persistent state (recent profiles, etc.)
    pub persistent: PersistentState,

    /// Path to persistent state file
    state_file: PathBuf,
}

impl TrayState {
    /// Create new tray state
    pub fn new() -> Result<Self> {
        let daemon_config = Self::load_daemon_config()?;
        let state_file = Self::state_file_path()?;
        let persistent = Self::load_persistent_state(&state_file)?;

        Ok(Self {
            status: ConnectionStatus::Disconnected,
            active_tunnels: HashMap::new(),
            last_heartbeat: None,
            daemon_config,
            persistent,
            state_file,
        })
    }

    /// Load daemon configuration
    fn load_daemon_config() -> Result<DaemonClientConfig> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get config directory"))?;
        let config_path = config_dir.join("ssh-tunnel-manager").join("cli.toml");

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let config = toml::from_str::<DaemonClientConfig>(&contents)?;
            Ok(config)
        } else {
            Ok(DaemonClientConfig::default())
        }
    }

    /// Get path to state file
    fn state_file_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get config directory"))?;
        let state_dir = config_dir.join("ssh-tunnel-manager");
        std::fs::create_dir_all(&state_dir)?;
        Ok(state_dir.join("tray-state.toml"))
    }

    /// Load persistent state from disk
    fn load_persistent_state(path: &PathBuf) -> Result<PersistentState> {
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            let state = toml::from_str(&contents)?;
            Ok(state)
        } else {
            Ok(PersistentState::default())
        }
    }

    /// Save persistent state to disk
    pub fn save(&self) -> Result<()> {
        let contents = toml::to_string_pretty(&self.persistent)?;
        std::fs::write(&self.state_file, contents)?;
        Ok(())
    }

    /// Update connection status based on current state
    pub fn update_status(&mut self) {
        self.status = if self.last_heartbeat.is_none() {
            ConnectionStatus::Disconnected
        } else if self.active_tunnels.is_empty() {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Active
        };
    }

    /// Add a recent profile
    pub fn add_recent_profile(&mut self, profile: &Profile) {
        // Remove existing entry if present
        self.persistent.recent_profiles.retain(|p| p.profile_id != profile.metadata.id);

        // Add to front
        self.persistent.recent_profiles.insert(0, RecentProfile {
            profile_id: profile.metadata.id,
            profile_name: profile.metadata.name.clone(),
            last_used: Utc::now(),
        });

        // Keep only last 10
        self.persistent.recent_profiles.truncate(10);

        // Save to disk
        let _ = self.save();
    }

    /// Get most recent profile
    pub fn get_recent_profile(&self) -> Option<&RecentProfile> {
        self.persistent.recent_profiles.first()
    }
}
