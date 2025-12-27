// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Daemon connection helpers
//!
//! This module provides helper functions for daemon operations.
//! The actual DaemonClient implementation remains in the GUI crates
//! to handle framework-specific async runtime integration.

use anyhow::{Result, Context};
use ssh_tunnel_common::DaemonClientConfig;

/// Load daemon client configuration from CLI config file
pub fn load_daemon_config() -> Result<DaemonClientConfig> {
    use std::fs;

    let config_path = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
        .join("ssh-tunnel-manager")
        .join("cli.toml");

    if !config_path.exists() {
        // Return default config if file doesn't exist
        return Ok(DaemonClientConfig::default());
    }

    let contents = fs::read_to_string(&config_path)
        .context("Failed to read CLI config file")?;

    // Parse the TOML - the CLI config wraps DaemonClientConfig
    #[derive(serde::Deserialize)]
    struct CliConfig {
        #[serde(flatten)]
        daemon_config: DaemonClientConfig,
    }

    let cli_config: CliConfig = toml::from_str(&contents)
        .context("Failed to parse CLI config file")?;

    Ok(cli_config.daemon_config)
}

/// Get CLI config file path
pub fn get_cli_config_path() -> Result<std::path::PathBuf> {
    let path = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
        .join("ssh-tunnel-manager")
        .join("cli.toml");

    Ok(path)
}

/// Get daemon config snippet path
pub fn get_daemon_config_snippet_path() -> Result<std::path::PathBuf> {
    ssh_tunnel_common::get_cli_config_snippet_path()
}

/// Check if daemon config snippet exists
pub fn daemon_config_snippet_exists() -> bool {
    ssh_tunnel_common::cli_config_snippet_exists()
}
