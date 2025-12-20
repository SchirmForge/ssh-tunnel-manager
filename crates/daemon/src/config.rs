// SSH Tunnel Manager - Daemon Config Module
// Handles daemon configuration (listener mode, TLS, auth, etc.)
// Profile management now in ssh-tunnel-common::profile_manager

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

/// Get the runtime directory for daemon state
pub fn runtime_dir() -> Result<PathBuf> {
    dirs::runtime_dir().ok_or_else(|| anyhow::anyhow!("Could not determine runtime directory"))
}

/// Get the socket path for the daemon
pub fn socket_path() -> Result<PathBuf> {
    Ok(runtime_dir()?.join("ssh-tunnel-manager.sock"))
}

/// Listener mode for the daemon
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ListenerMode {
    /// Unix domain socket (local-only, no TLS)
    UnixSocket,
    /// TCP with HTTP (localhost-only, no TLS)
    TcpHttp,
    /// TCP with HTTPS/TLS (network-ready, secure)
    TcpHttps,
}

impl Default for ListenerMode {
    fn default() -> Self {
        ListenerMode::UnixSocket
    }
}

/// Daemon configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaemonConfig {
    /// Listener mode (UnixSocket, TcpHttp, or TcpHttps)
    #[serde(default)]
    pub listener_mode: ListenerMode,

    /// Bind address for TCP modes (e.g., "0.0.0.0:3443" or "127.0.0.1:3443")
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    /// Path to TLS certificate file (for TcpHttps mode)
    #[serde(default = "default_tls_cert_path")]
    pub tls_cert_path: PathBuf,

    /// Path to TLS private key file (for TcpHttps mode)
    #[serde(default = "default_tls_key_path")]
    pub tls_key_path: PathBuf,

    /// Path to authentication token file
    #[serde(default = "default_auth_token_path")]
    pub auth_token_path: PathBuf,

    /// Require authentication (recommended for TCP modes, optional for UnixSocket)
    #[serde(default = "default_require_auth")]
    pub require_auth: bool,

    /// Path to SSH known_hosts file
    /// Default: ~/.config/ssh-tunnel-manager/known_hosts
    /// Can also use system known_hosts: ~/.ssh/known_hosts
    #[serde(default = "default_known_hosts_path")]
    pub known_hosts_path: PathBuf,

    /// Enable group access to Unix socket and runtime directory
    /// When true, sets permissions to 0770/0660 instead of 0700/0600
    /// Useful for system daemons where multiple users need access via a shared group
    /// Default: false (restrictive permissions for single-user security)
    #[serde(default = "default_group_access")]
    pub group_access: bool,
}

fn default_bind_address() -> String {
    "127.0.0.1:3443".to_string()
}

fn default_tls_cert_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ssh-tunnel-manager")
        .join("server.crt")
}

fn default_tls_key_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ssh-tunnel-manager")
        .join("server.key")
}

fn default_auth_token_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ssh-tunnel-manager")
        .join("daemon.token")
}

fn default_require_auth() -> bool {
    true // Default to true for security
}

fn default_known_hosts_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ssh-tunnel-manager")
        .join("known_hosts")
}

fn default_group_access() -> bool {
    false // Default to restrictive single-user permissions
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            listener_mode: ListenerMode::default(),
            bind_address: default_bind_address(),
            tls_cert_path: default_tls_cert_path(),
            tls_key_path: default_tls_key_path(),
            auth_token_path: default_auth_token_path(),
            require_auth: default_require_auth(),
            known_hosts_path: default_known_hosts_path(),
            group_access: default_group_access(),
        }
    }
}

impl DaemonConfig {
    /// Validate the daemon configuration
    pub fn validate(&self) -> Result<()> {
        // For TCP modes, check if bind address is non-loopback
        if matches!(self.listener_mode, ListenerMode::TcpHttp | ListenerMode::TcpHttps) {
            // Parse the bind address to check if it's loopback
            let is_loopback = self.bind_address.starts_with("127.")
                || self.bind_address.starts_with("localhost:")
                || self.bind_address == "localhost";

            // If non-loopback and not HTTPS, reject
            if !is_loopback && self.listener_mode == ListenerMode::TcpHttp {
                anyhow::bail!(
                    "Security violation: Non-loopback TCP connections (bind_address: {}) require HTTPS mode.\n\
                     Current mode: TcpHttp\n\
                     \n\
                     To fix this:\n\
                     1. Change listener_mode to 'tcp-https' in daemon.toml, OR\n\
                     2. Use a loopback address (127.0.0.1 or localhost) for bind_address\n\
                     \n\
                     HTTP mode is only allowed for localhost connections due to lack of encryption.",
                    self.bind_address
                );
            }
        }

        Ok(())
    }

    /// Load daemon configuration from file
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            info!("No daemon configuration found, using defaults");
            info!("Configuration will be saved to: {}", config_path.display());
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let contents = fs::read_to_string(&config_path)
            .context("Failed to read daemon configuration")?;

        let config: Self = toml::from_str(&contents)
            .context("Failed to parse daemon configuration")?;

        // Validate the loaded configuration
        config.validate()
            .context("Configuration validation failed")?;

        info!("Loaded daemon configuration from: {}", config_path.display());
        Ok(config)
    }

    /// Save daemon configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Ensure directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create configuration directory")?;
        }

        let contents = toml::to_string_pretty(self)
            .context("Failed to serialize daemon configuration")?;

        fs::write(&config_path, contents)
            .context("Failed to write daemon configuration")?;

        // Set restrictive permissions on config file (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&config_path, permissions)
                .context("Failed to set config file permissions")?;
        }

        info!("Saved daemon configuration to: {}", config_path.display());
        Ok(())
    }

    /// Get the path to the daemon configuration file
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("ssh-tunnel-manager").join("daemon.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_unix_socket_mode() {
        let config = DaemonConfig {
            listener_mode: ListenerMode::UnixSocket,
            bind_address: "127.0.0.1:3443".to_string(),
            ..Default::default()
        };
        // Unix socket mode should always pass validation
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_tcp_http_loopback() {
        let config = DaemonConfig {
            listener_mode: ListenerMode::TcpHttp,
            bind_address: "127.0.0.1:3443".to_string(),
            ..Default::default()
        };
        // Loopback addresses should be allowed for tcp-http
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_tcp_http_localhost() {
        let config = DaemonConfig {
            listener_mode: ListenerMode::TcpHttp,
            bind_address: "localhost:3443".to_string(),
            ..Default::default()
        };
        // localhost should be allowed for tcp-http
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_tcp_http_non_loopback_rejected() {
        let config = DaemonConfig {
            listener_mode: ListenerMode::TcpHttp,
            bind_address: "0.0.0.0:3443".to_string(),
            ..Default::default()
        };
        // Non-loopback addresses should be rejected for tcp-http
        assert!(config.validate().is_err());

        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("Security violation"));
        assert!(err.to_string().contains("require HTTPS"));
    }

    #[test]
    fn test_validate_tcp_http_network_address_rejected() {
        let config = DaemonConfig {
            listener_mode: ListenerMode::TcpHttp,
            bind_address: "192.168.1.100:3443".to_string(),
            ..Default::default()
        };
        // Network addresses should be rejected for tcp-http
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_tcp_https_non_loopback() {
        let config = DaemonConfig {
            listener_mode: ListenerMode::TcpHttps,
            bind_address: "0.0.0.0:3443".to_string(),
            ..Default::default()
        };
        // Non-loopback addresses should be allowed for tcp-https
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_tcp_https_loopback() {
        let config = DaemonConfig {
            listener_mode: ListenerMode::TcpHttps,
            bind_address: "127.0.0.1:3443".to_string(),
            ..Default::default()
        };
        // Loopback addresses should also work for tcp-https
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_default_require_auth_is_true() {
        let config = DaemonConfig::default();
        // Verify that require_auth defaults to true for security
        assert_eq!(config.require_auth, true);
    }
}

/// Write CLI config snippet to help users configure their CLI
pub fn write_cli_config_snippet(
    listener_mode: &ListenerMode,
    bind_address: &str,
    auth_token: Option<&str>,
    tls_fingerprint: Option<&str>,
) -> Result<()> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
    let snippet_path = config_dir
        .join("ssh-tunnel-manager")
        .join("cli-config.snippet");

    // Ensure parent directory exists
    if let Some(parent) = snippet_path.parent() {
        fs::create_dir_all(parent).context("Failed to create config directory")?;
    }

    // Get the actual socket path being used
    let socket_path_str = socket_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "auto-detect".to_string());

    // Build the CLI config based on listener mode
    let config_content = match listener_mode {
        ListenerMode::UnixSocket => {
            format!(
                "# CLI Configuration for SSH Tunnel Manager\n\
                 # Copy this to ~/.config/ssh-tunnel-manager/cli.toml\n\
                 \n\
                 connection_mode = \"unix-socket\"\n\
                 # Socket path (auto-detected by default): {}\n\
                 # Uncomment to override:\n\
                 # daemon_url = \"{}\"\n",
                socket_path_str, socket_path_str
            )
        }
        ListenerMode::TcpHttp => {
            let mut content = format!(
                "# CLI Configuration for SSH Tunnel Manager\n\
                 # Copy this to ~/.config/ssh-tunnel-manager/cli.toml\n\
                 \n\
                 connection_mode = \"http\"\n\
                 daemon_url = \"{}\"\n",
                bind_address
            );
            if let Some(token) = auth_token {
                content.push_str(&format!("auth_token = \"{}\"\n", token));
            }
            content
        }
        ListenerMode::TcpHttps => {
            let mut content = format!(
                "# CLI Configuration for SSH Tunnel Manager\n\
                 # Copy this to ~/.config/ssh-tunnel-manager/cli.toml\n\
                 \n\
                 connection_mode = \"https\"\n\
                 daemon_url = \"{}\"\n",
                bind_address
            );
            if let Some(token) = auth_token {
                content.push_str(&format!("auth_token = \"{}\"\n", token));
            }
            if let Some(fingerprint) = tls_fingerprint {
                content.push_str(&format!("tls_cert_fingerprint = \"{}\"\n", fingerprint));
            }
            content
        }
    };

    // Write the snippet file
    fs::write(&snippet_path, config_content)
        .context("Failed to write CLI config snippet")?;

    info!("");
    info!("═══════════════════════════════════════════════════════════");
    info!("📋 CLI Configuration Snippet Generated");
    info!("═══════════════════════════════════════════════════════════");
    info!("");
    info!("A configuration file has been created at:");
    info!("  {}", snippet_path.display());
    info!("");
    info!("To configure your CLI, run:");
    info!("  cp {} ~/.config/ssh-tunnel-manager/cli.toml",
        snippet_path.display());
    info!("");
    info!("═══════════════════════════════════════════════════════════");
    info!("");

    Ok(())
}
