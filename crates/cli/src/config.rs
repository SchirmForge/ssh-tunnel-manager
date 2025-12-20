// SSH Tunnel Manager - CLI Config Module
// Handles CLI configuration for connecting to the daemon
// Note: Most daemon client logic moved to ssh-tunnel-common::daemon_client

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub use ssh_tunnel_common::DaemonClientConfig;

/// CLI configuration (wrapper around DaemonClientConfig with file I/O)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CliConfig {
    #[serde(flatten)]
    pub daemon_config: DaemonClientConfig,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            daemon_config: DaemonClientConfig::default(),
        }
    }
}

impl CliConfig {
    /// Load CLI configuration from file
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            // Return default config if file doesn't exist
            return Ok(Self::default());
        }

        let contents =
            fs::read_to_string(&config_path).context("Failed to read CLI configuration")?;

        let config: Self =
            toml::from_str(&contents).context("Failed to parse CLI configuration")?;

        Ok(config)
    }

    /// Get the path to the CLI configuration file
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("ssh-tunnel-manager").join("cli.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CliConfig::default();
        assert_eq!(
            config.daemon_config.connection_mode,
            ConnectionMode::UnixSocket
        );
        assert!(!config.daemon_config.daemon_url.is_empty());
    }

    #[test]
    fn test_daemon_base_url() {
        let mut config = CliConfig::default();

        // Unix socket mode
        config.daemon_config.connection_mode = ConnectionMode::UnixSocket;
        assert_eq!(
            config.daemon_config.daemon_base_url().unwrap(),
            "http://daemon"
        );

        // HTTPS mode (URL without protocol)
        config.daemon_config.connection_mode = ConnectionMode::Https;
        config.daemon_config.daemon_url = "example.com:3443".to_string();
        assert_eq!(
            config.daemon_config.daemon_base_url().unwrap(),
            "https://example.com:3443"
        );
    }
}
