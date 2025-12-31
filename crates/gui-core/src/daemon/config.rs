// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Daemon connection configuration helpers

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

/// Configuration status for first-launch detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigStatus {
    /// Configuration file exists and is valid
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
    use std::fs;

    let snippet_path = get_daemon_config_snippet_path()?;

    if !snippet_path.exists() {
        anyhow::bail!("Configuration snippet does not exist");
    }

    let contents = fs::read_to_string(&snippet_path)
        .context("Failed to read configuration snippet")?;

    let config: DaemonClientConfig = toml::from_str(&contents)
        .context("Failed to parse configuration snippet")?;

    Ok(config)
}

/// Save daemon configuration to cli.toml
pub fn save_daemon_config(config: &DaemonClientConfig) -> Result<()> {
    use std::fs;

    // Validate configuration before saving
    ssh_tunnel_common::validate_client_config(config)?;

    let config_path = get_cli_config_path()?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .context("Failed to create config directory")?;
    }

    // Wrap in CliConfig structure for proper serialization
    #[derive(serde::Serialize)]
    struct CliConfig {
        #[serde(flatten)]
        daemon_config: DaemonClientConfig,
    }

    let cli_config = CliConfig {
        daemon_config: config.clone(),
    };

    let toml_content = toml::to_string_pretty(&cli_config)
        .context("Failed to serialize configuration")?;

    fs::write(&config_path, toml_content)
        .context("Failed to write configuration file")?;

    // Set restrictive permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&config_path, permissions)
            .context("Failed to set config file permissions")?;
    }

    Ok(())
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
