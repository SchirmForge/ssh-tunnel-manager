// Configuration structures for SSH Tunnel Manager

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::types::{AuthType, ForwardingType};

/// Complete tunnel profile configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    #[serde(flatten)]
    pub metadata: ProfileMetadata,
    pub connection: ConnectionConfig,
    pub forwarding: ForwardingConfig,
    #[serde(default)]
    pub options: TunnelOptions,
}

/// Profile metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetadata {
    /// Unique profile identifier
    pub id: Uuid,
    /// Human-readable profile name
    pub name: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Profile creation timestamp
    pub created_at: DateTime<Utc>,
    /// Profile last modification timestamp
    pub modified_at: DateTime<Utc>,
    /// Optional tags for organization
    #[serde(default)]
    pub tags: Vec<String>,
}

/// SSH connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// SSH server hostname or IP
    pub host: String,
    /// SSH server port (default: 22)
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    /// SSH username
    pub user: String,
    /// Authentication type
    pub auth_type: AuthType,
    /// Path to SSH private key (for key auth)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_path: Option<PathBuf>,
    /// Whether password is stored in keyring
    #[serde(default)]
    pub password_stored: bool,
}

/// Port forwarding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardingConfig {
    /// Type of forwarding
    #[serde(rename = "type")]
    pub forwarding_type: ForwardingType,
    /// Local port to bind
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_port: Option<u16>,
    /// Remote host to forward to (for local/remote forwarding)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_host: Option<String>,
    /// Remote port to forward to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_port: Option<u16>,
    /// Local bind address (default: 127.0.0.1)
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
}

/// Tunnel options and behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelOptions {
    /// Enable SSH compression
    #[serde(default)]
    pub compression: bool,
    /// Keepalive interval in seconds (0 = disabled)
    #[serde(default = "default_keepalive")]
    pub keepalive_interval: u64,
    /// Enable automatic reconnection on failure
    #[serde(default = "default_auto_reconnect")]
    pub auto_reconnect: bool,
    /// Maximum reconnection attempts (0 = unlimited)
    #[serde(default = "default_reconnect_attempts")]
    pub reconnect_attempts: u32,
    /// Delay between reconnection attempts in seconds
    #[serde(default = "default_reconnect_delay")]
    pub reconnect_delay: u64,
    /// Enable TCP keepalive on forwarded connections
    #[serde(default)]
    pub tcp_keepalive: bool,
    /// Maximum SSH packet size in bytes
    #[serde(default = "default_max_packet_size")]
    pub max_packet_size: u32,
    /// SSH window size in bytes
    #[serde(default = "default_window_size")]
    pub window_size: u32,
}

// Default value functions
fn default_ssh_port() -> u16 {
    22
}

fn default_bind_address() -> String {
    "127.0.0.1".to_string()
}

fn default_keepalive() -> u64 {
    60
}

fn default_auto_reconnect() -> bool {
    true
}

fn default_reconnect_attempts() -> u32 {
    3
}

fn default_reconnect_delay() -> u64 {
    5
}

fn default_max_packet_size() -> u32 {
    65535 // 64 KiB - 1 (TCP packet size is usually set to 65535 and this size cannot be higher)
}

fn default_window_size() -> u32 {
    2097152 // 2 MiB
}

impl Default for TunnelOptions {
    fn default() -> Self {
        Self {
            compression: false,
            keepalive_interval: default_keepalive(),
            auto_reconnect: default_auto_reconnect(),
            reconnect_attempts: default_reconnect_attempts(),
            reconnect_delay: default_reconnect_delay(),
            tcp_keepalive: false,
            max_packet_size: default_max_packet_size(),
            window_size: default_window_size(),
        }
    }
}

impl Profile {
    /// Create a new profile with the given name and configuration
    pub fn new(name: String, connection: ConnectionConfig, forwarding: ForwardingConfig) -> Self {
        let now = Utc::now();
        Self {
            metadata: ProfileMetadata {
                id: Uuid::new_v4(),
                name,
                description: None,
                created_at: now,
                modified_at: now,
                tags: Vec::new(),
            },
            connection,
            forwarding,
            options: TunnelOptions::default(),
        }
    }

    /// Create a new profile with custom options
    pub fn new_with_options(
        name: String,
        connection: ConnectionConfig,
        forwarding: ForwardingConfig,
        options: TunnelOptions,
    ) -> Self {
        let now = Utc::now();
        Self {
            metadata: ProfileMetadata {
                id: Uuid::new_v4(),
                name,
                description: None,
                created_at: now,
                modified_at: now,
                tags: Vec::new(),
            },
            connection,
            forwarding,
            options,
        }
    }

    /// Validate the profile configuration
    pub fn validate(&self) -> Result<()> {
        // Validate connection
        if self.connection.host.is_empty() {
            return Err(Error::Config("Host cannot be empty".to_string()));
        }
        if self.connection.user.is_empty() {
            return Err(Error::Config("User cannot be empty".to_string()));
        }
        if self.connection.port == 0 {
            return Err(Error::Config("Port must be greater than 0".to_string()));
        }

        // Validate auth configuration
        if self.connection.auth_type == AuthType::Key && self.connection.key_path.is_none() {
            return Err(Error::Config(
                "Key path required for key authentication".to_string(),
            ));
        }

        // Validate forwarding configuration
        match self.forwarding.forwarding_type {
            ForwardingType::Local | ForwardingType::Remote => {
                if self.forwarding.local_port.is_none() {
                    return Err(Error::Config("Local port required".to_string()));
                }
                if self.forwarding.remote_host.is_none() {
                    return Err(Error::Config("Remote host required".to_string()));
                }
                if self.forwarding.remote_port.is_none() {
                    return Err(Error::Config("Remote port required".to_string()));
                }
            }
            ForwardingType::Dynamic => {
                if self.forwarding.local_port.is_none() {
                    return Err(Error::Config(
                        "Local port required for dynamic forwarding".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Get the configuration file path for this profile
    pub fn config_path(&self) -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| Error::Config("Could not determine config directory".to_string()))?;

        let profile_dir = config_dir.join("ssh-tunnel-manager").join("profiles");
        Ok(profile_dir.join(format!("{}.toml", self.metadata.id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_validation() {
        let profile = Profile::new(
            "test".to_string(),
            ConnectionConfig {
                host: "example.com".to_string(),
                port: 22,
                user: "user".to_string(),
                auth_type: AuthType::Key,
                key_path: Some(PathBuf::from("/home/user/.ssh/id_ed25519")),
                password_stored: false,
            },
            ForwardingConfig {
                forwarding_type: ForwardingType::Local,
                local_port: Some(5432),
                remote_host: Some("db.internal".to_string()),
                remote_port: Some(5432),
                bind_address: "127.0.0.1".to_string(),
            },
        );

        assert!(profile.validate().is_ok());
    }

    #[test]
    fn test_invalid_profile_empty_host() {
        let profile = Profile::new(
            "test".to_string(),
            ConnectionConfig {
                host: "".to_string(),
                port: 22,
                user: "user".to_string(),
                auth_type: AuthType::Key,
                key_path: Some(PathBuf::from("/home/user/.ssh/id_ed25519")),
                password_stored: false,
            },
            ForwardingConfig {
                forwarding_type: ForwardingType::Local,
                local_port: Some(5432),
                remote_host: Some("db.internal".to_string()),
                remote_port: Some(5432),
                bind_address: "127.0.0.1".to_string(),
            },
        );

        assert!(profile.validate().is_err());
    }
}
