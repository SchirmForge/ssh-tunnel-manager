// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Daemon connection configuration helpers

use anyhow::{Context, Result};
use ssh_tunnel_common::DaemonClientConfig;

use crate::client_setup::ClientSetupRepository;

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

    let contents = fs::read_to_string(&config_path).context("Failed to read CLI config file")?;

    // Parse the TOML - the CLI config wraps DaemonClientConfig
    #[derive(serde::Deserialize)]
    struct CliConfig {
        #[serde(flatten)]
        daemon_config: DaemonClientConfig,
    }

    let cli_config: CliConfig =
        toml::from_str(&contents).context("Failed to parse CLI config file")?;

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

/// Configuration status for first-launch detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigStatus {
    /// Configuration file exists (legacy compatibility; contents are not inspected here)
    Exists,
    /// No config file, but a daemon-generated snippet is available
    SnippetAvailable,
    /// No config file and no snippet - manual setup required
    NeedsSetup,
}

/// Check configuration status
pub fn check_config_status() -> ConfigStatus {
    let config_path = match get_cli_config_path() {
        Ok(path) => path,
        Err(_) => return ConfigStatus::NeedsSetup,
    };

    if config_path.exists() {
        ConfigStatus::Exists
    } else if daemon_config_snippet_exists() {
        ConfigStatus::SnippetAvailable
    } else {
        ConfigStatus::NeedsSetup
    }
}

/// Load snippet configuration from daemon-generated file
pub fn load_snippet_config() -> Result<DaemonClientConfig> {
    let repository = ClientSetupRepository::for_default_paths()?;
    let draft = repository.load_snippet()?;
    let auth_token = draft.authentication_token().to_string();
    Ok(DaemonClientConfig {
        connection_mode: draft.connection_mode,
        daemon_host: draft.daemon_host,
        daemon_port: draft.daemon_port.parse().unwrap_or(3443),
        daemon_url: draft.daemon_url,
        auth_token,
        tls_cert_fingerprint: draft.tls_cert_fingerprint,
        skip_ssh_setup_warning: draft.skip_ssh_setup_warning,
    })
}

/// Save daemon configuration to cli.toml
pub fn save_daemon_config(config: &DaemonClientConfig) -> Result<()> {
    ClientSetupRepository::for_default_paths()?.persist_config(config)
}

/// Save the skip SSH setup warning preference to config file
/// This updates only the skip_ssh_setup_warning field while preserving other settings
pub async fn save_skip_ssh_warning_preference(skip: bool) -> Result<()> {
    // Load current config
    let mut config = load_daemon_config()?;

    // Update the preference
    config.skip_ssh_setup_warning = skip;

    // Save back to file
    save_daemon_config(&config)
}
