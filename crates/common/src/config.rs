// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

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

/// Where password/passphrase is stored
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PasswordStorage {
    /// Not stored - user will be prompted
    #[default]
    None,
    /// Legacy value meaning "the daemon reads it from its own keychain".
    ///
    /// Ambiguous by construction: it only says *a* keychain, and which machine that is
    /// depends on where the daemon runs. Against a remote daemon the credential was written
    /// to the client's keychain and looked for on the daemon's, so nothing was found and the
    /// user was prompted anyway — a silent no-op.
    ///
    /// Still read, because profiles are TOML a user may have copied or backed up, but no
    /// longer written. [`resolved`](Self::resolved) turns it into [`Self::DaemonHost`] or
    /// [`Self::Client`] once the daemon's location is known.
    Keychain,
    /// Stored in the **daemon host's** keychain, and read there by the daemon.
    ///
    /// Only offered for a local daemon: see `.plan/AUTH-01_credential-storage.md` §9.1.
    DaemonHost,
    /// Stored on the **client**, and sent to the daemon when it asks.
    ///
    /// Works the same whether the daemon is local or remote, because the credential lives
    /// where the human and the unlocked keyring are. The daemon is unchanged: it raises its
    /// usual prompt, and the client answers from its store instead of asking a person.
    Client,
    /// Stored in a file on the daemon host (unattended operation; not yet implemented)
    File,
}

impl PasswordStorage {
    /// Resolve the legacy [`Self::Keychain`] value now that the daemon's location is known.
    ///
    /// `Keychain` meant "read it from the daemon's keychain", which is only coherent when the
    /// daemon is on this machine. For a remote daemon the user's intent was "remember this
    /// for me" — which is [`Self::Client`], the one that actually works there.
    ///
    /// Everything else passes through, so this is safe to call on any value.
    pub fn resolved(self, daemon_is_local: bool) -> Self {
        match self {
            Self::Keychain if daemon_is_local => Self::DaemonHost,
            Self::Keychain => Self::Client,
            other => other,
        }
    }

    /// Whether a credential is kept by the client and sent when the daemon asks.
    pub fn is_client_held(self) -> bool {
        matches!(self, Self::Client)
    }

    /// Whether the daemon reads the credential from its own keychain.
    ///
    /// Includes the legacy value, since the daemon is by definition local to itself.
    pub fn is_daemon_held(self) -> bool {
        matches!(self, Self::DaemonHost | Self::Keychain)
    }
}

impl Serialize for PasswordStorage {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            PasswordStorage::None => serializer.serialize_str("none"),
            PasswordStorage::Keychain => serializer.serialize_str("keychain"),
            PasswordStorage::DaemonHost => serializer.serialize_str("daemon-host"),
            PasswordStorage::Client => serializer.serialize_str("client"),
            PasswordStorage::File => serializer.serialize_str("file"),
        }
    }
}

impl<'de> Deserialize<'de> for PasswordStorage {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PasswordStorageVisitor;

        impl<'de> serde::de::Visitor<'de> for PasswordStorageVisitor {
            type Value = PasswordStorage;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a password storage type (none/keychain/file or boolean)")
            }

            fn visit_bool<E>(self, v: bool) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // Backward compatibility: old boolean values
                Ok(if v {
                    PasswordStorage::Keychain
                } else {
                    PasswordStorage::None
                })
            }

            fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v.to_lowercase().as_str() {
                    "none" | "false" => Ok(PasswordStorage::None),
                    "keychain" | "true" => Ok(PasswordStorage::Keychain),
                    "daemon-host" | "daemon_host" => Ok(PasswordStorage::DaemonHost),
                    "client" => Ok(PasswordStorage::Client),
                    "file" => Ok(PasswordStorage::File),
                    _ => Err(E::custom(format!("unknown password storage type: {}", v))),
                }
            }
        }

        deserializer.deserialize_any(PasswordStorageVisitor)
    }
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
    /// Where password/passphrase is stored
    #[serde(default)]
    pub password_storage: PasswordStorage,
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
                password_storage: PasswordStorage::None,
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
                password_storage: PasswordStorage::None,
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

#[cfg(test)]
mod password_storage_tests {
    use super::PasswordStorage;

    /// The legacy value only ever said "a keychain", never whose. Which machine that meant
    /// depended on where the daemon ran, so it is resolved once that is known.
    #[test]
    fn the_legacy_value_resolves_by_where_the_daemon_is() {
        assert_eq!(
            PasswordStorage::Keychain.resolved(true),
            PasswordStorage::DaemonHost,
            "a local daemon reads its own keychain, which is also the client's"
        );
        assert_eq!(
            PasswordStorage::Keychain.resolved(false),
            PasswordStorage::Client,
            "against a remote daemon the credential is on the client, so say so"
        );
    }

    #[test]
    fn every_other_value_passes_through_resolution_unchanged() {
        for value in [
            PasswordStorage::None,
            PasswordStorage::Client,
            PasswordStorage::DaemonHost,
            PasswordStorage::File,
        ] {
            assert_eq!(value.resolved(true), value);
            assert_eq!(value.resolved(false), value);
        }
    }

    /// The daemon is by definition local to itself, so it honours the legacy value.
    #[test]
    fn the_daemon_treats_the_legacy_value_as_its_own() {
        assert!(PasswordStorage::Keychain.is_daemon_held());
        assert!(PasswordStorage::DaemonHost.is_daemon_held());
        assert!(!PasswordStorage::Client.is_daemon_held());
        assert!(!PasswordStorage::None.is_daemon_held());
    }

    #[test]
    fn only_client_storage_is_client_held() {
        assert!(PasswordStorage::Client.is_client_held());
        for other in [
            PasswordStorage::None,
            PasswordStorage::Keychain,
            PasswordStorage::DaemonHost,
            PasswordStorage::File,
        ] {
            assert!(!other.is_client_held());
        }
    }

    /// Profiles are TOML a user may have copied or backed up, so the legacy spellings must
    /// keep parsing — including the boolean form that predates v0.1.6.
    #[test]
    fn legacy_spellings_still_parse() {
        for (text, expected) in [
            ("true", PasswordStorage::Keychain),
            ("false", PasswordStorage::None),
            ("\"keychain\"", PasswordStorage::Keychain),
            ("\"none\"", PasswordStorage::None),
            ("\"client\"", PasswordStorage::Client),
            ("\"daemon-host\"", PasswordStorage::DaemonHost),
            ("\"file\"", PasswordStorage::File),
        ] {
            let parsed: PasswordStorage = toml::from_str(&format!("v = {text}"))
                .map(|w: Wrapper| w.v)
                .unwrap();
            assert_eq!(parsed, expected, "{text} should parse as {expected:?}");
        }
    }

    #[derive(serde::Deserialize)]
    struct Wrapper {
        v: PasswordStorage,
    }

    /// Round-trips, so a profile rewritten by a newer version is still readable.
    #[test]
    fn the_new_values_round_trip() {
        for value in [
            PasswordStorage::None,
            PasswordStorage::Client,
            PasswordStorage::DaemonHost,
            PasswordStorage::File,
        ] {
            let text = toml::to_string(&Wrapper2 { v: value }).unwrap();
            let back: Wrapper2 = toml::from_str(&text).unwrap();
            assert_eq!(back.v, value);
        }
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Wrapper2 {
        v: PasswordStorage,
    }
}
