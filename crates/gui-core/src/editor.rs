// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Toolkit-neutral profile editor and credential update contracts.
//!
//! Existing credentials never enter an editor draft.  The presentation layer
//! receives only a presence/status marker and submits an explicit keep, replace,
//! or remove operation alongside the profile.

use std::fmt;
use std::path::PathBuf;

use ssh_tunnel_common::{
    AuthType, ConnectionConfig, ForwardingConfig, ForwardingType, PasswordStorage, Profile,
    TunnelOptions, Utc,
};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Secret supplied by a user while editing a profile.
///
/// `Debug` is deliberately redacted because commands and effects may be logged
/// while diagnosing the GUI/runtime bridge.
pub struct SecretValue(Zeroizing<String>);

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(Zeroizing::new(value.into()))
    }

    pub fn expose(&self) -> &str {
        self.0.as_str()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Clone for SecretValue {
    fn clone(&self) -> Self {
        Self::new(self.expose())
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretValue([REDACTED])")
    }
}

/// How the credential associated with a profile UUID should change.
#[derive(Clone)]
pub enum CredentialUpdate {
    /// Preserve the current entry without reading it into the editor.
    Keep,
    /// Store or replace the entry with a value entered in this editor session.
    Store(SecretValue),
    /// Remove the client-held entry.
    Remove,
}

impl fmt::Debug for CredentialUpdate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keep => formatter.write_str("Keep"),
            Self::Store(_) => formatter.write_str("Store([REDACTED])"),
            Self::Remove => formatter.write_str("Remove"),
        }
    }
}

/// Complete, validated request passed from an editor to the shared runtime.
#[derive(Debug, Clone)]
pub struct ProfileSaveRequest {
    pub profile: Profile,
    pub overwrite: bool,
    pub credential: CredentialUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileEditorMode {
    Create,
    Edit,
    Duplicate,
}

/// What the client can safely say about an existing credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredCredentialState {
    NotStored,
    Stored,
    Unavailable(String),
}

/// Editor bootstrap data prepared away from the GTK main context.
#[derive(Debug, Clone)]
pub struct ProfileEditorSession {
    pub mode: ProfileEditorMode,
    pub draft: ProfileEditorDraft,
    pub client_credential: StoredCredentialState,
    pub daemon_is_local: bool,
}

/// Deletion confirmation copy prepared from structured profile data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDeletionRequest {
    pub profile_id: Uuid,
    pub profile_name: String,
}

/// Post-save choice offered when an edited profile is currently in use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileReconnectRequest {
    pub profile_id: Uuid,
    pub profile_name: String,
}

/// An editable profile with text fields kept as text until submission.
///
/// Keeping ports and numeric tuning values as strings lets every toolkit show
/// incomplete user input without inventing placeholder values.  `template`
/// retains metadata and any schema fields not currently exposed by the editor.
#[derive(Debug, Clone)]
pub struct ProfileEditorDraft {
    template: Profile,
    key_path_is_local: bool,
    pub name: String,
    pub host: String,
    pub ssh_port: String,
    pub user: String,
    pub forwarding_type: ForwardingType,
    pub bind_address: String,
    pub local_port: String,
    pub remote_host: String,
    pub remote_port: String,
    pub auth_type: AuthType,
    pub key_path: String,
    pub password_storage: PasswordStorage,
    pub compression: bool,
    pub keepalive_interval: String,
    pub auto_reconnect: bool,
    pub reconnect_attempts: String,
    pub reconnect_delay: String,
    pub tcp_keepalive: bool,
    pub max_packet_size: String,
    pub window_size: String,
}

impl ProfileEditorDraft {
    pub fn new(daemon_is_local: bool) -> Self {
        let profile = Profile::new(
            String::new(),
            ConnectionConfig {
                host: String::new(),
                port: 22,
                user: String::new(),
                auth_type: AuthType::Key,
                key_path: None,
                password_storage: PasswordStorage::None,
            },
            ForwardingConfig {
                forwarding_type: ForwardingType::Local,
                local_port: None,
                remote_host: None,
                remote_port: None,
                bind_address: "127.0.0.1".to_string(),
            },
        );
        Self::from_profile(profile, daemon_is_local)
    }

    pub fn edit(profile: Profile, daemon_is_local: bool) -> Self {
        Self::from_profile(profile, daemon_is_local)
    }

    pub fn duplicate(source: &Profile, name: String, daemon_is_local: bool) -> Self {
        let mut profile = source.clone();
        let now = Utc::now();
        profile.metadata.id = Uuid::new_v4();
        profile.metadata.name = name;
        profile.metadata.created_at = now;
        profile.metadata.modified_at = now;
        // A credential is scoped to the original UUID and is never copied
        // implicitly.  The duplicate editor may explicitly store a new one.
        profile.connection.password_storage = PasswordStorage::None;
        Self::from_profile(profile, daemon_is_local)
    }

    fn from_profile(mut profile: Profile, daemon_is_local: bool) -> Self {
        profile.connection.password_storage = profile
            .connection
            .password_storage
            .resolved(daemon_is_local);
        Self {
            name: profile.metadata.name.clone(),
            host: profile.connection.host.clone(),
            ssh_port: profile.connection.port.to_string(),
            user: profile.connection.user.clone(),
            forwarding_type: profile.forwarding.forwarding_type.clone(),
            bind_address: profile.forwarding.bind_address.clone(),
            local_port: profile
                .forwarding
                .local_port
                .map(|value| value.to_string())
                .unwrap_or_default(),
            remote_host: profile.forwarding.remote_host.clone().unwrap_or_default(),
            remote_port: profile
                .forwarding
                .remote_port
                .map(|value| value.to_string())
                .unwrap_or_default(),
            auth_type: profile.connection.auth_type.clone(),
            key_path: profile
                .connection
                .key_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            password_storage: profile.connection.password_storage,
            compression: profile.options.compression,
            keepalive_interval: profile.options.keepalive_interval.to_string(),
            auto_reconnect: profile.options.auto_reconnect,
            reconnect_attempts: profile.options.reconnect_attempts.to_string(),
            reconnect_delay: profile.options.reconnect_delay.to_string(),
            tcp_keepalive: profile.options.tcp_keepalive,
            max_packet_size: profile.options.max_packet_size.to_string(),
            window_size: profile.options.window_size.to_string(),
            key_path_is_local: daemon_is_local,
            template: profile,
        }
    }

    pub fn profile_id(&self) -> Uuid {
        self.template.metadata.id
    }

    pub fn build(&self) -> Result<Profile, EditorValidationErrors> {
        let mut errors = Vec::new();

        let name = required(&self.name, EditorField::Name, "Profile name", &mut errors);
        let host = required(&self.host, EditorField::Host, "SSH host", &mut errors);
        let user = required(&self.user, EditorField::User, "SSH user", &mut errors);
        let ssh_port = parse_number::<u16>(
            &self.ssh_port,
            EditorField::SshPort,
            "SSH port",
            true,
            &mut errors,
        );
        let local_port = parse_number::<u16>(
            &self.local_port,
            EditorField::LocalPort,
            "Local port",
            true,
            &mut errors,
        );
        let bind_address = required(
            &self.bind_address,
            EditorField::BindAddress,
            "Bind address",
            &mut errors,
        );

        let (remote_host, remote_port) = match self.forwarding_type {
            ForwardingType::Local | ForwardingType::Remote => (
                Some(required(
                    &self.remote_host,
                    EditorField::RemoteHost,
                    "Remote host",
                    &mut errors,
                )),
                parse_number::<u16>(
                    &self.remote_port,
                    EditorField::RemotePort,
                    "Remote port",
                    true,
                    &mut errors,
                ),
            ),
            ForwardingType::Dynamic => (
                self.template.forwarding.remote_host.clone(),
                self.template.forwarding.remote_port,
            ),
        };

        let key_path = if self.auth_type == AuthType::Key {
            let value = required(
                &self.key_path,
                EditorField::KeyPath,
                "SSH key path",
                &mut errors,
            );
            (!value.is_empty()).then(|| {
                if self.key_path_is_local {
                    expand_home_path(&value)
                } else {
                    PathBuf::from(value)
                }
            })
        } else {
            None
        };

        let keepalive_interval = parse_number::<u64>(
            &self.keepalive_interval,
            EditorField::KeepaliveInterval,
            "Keepalive interval",
            false,
            &mut errors,
        );
        let reconnect_attempts = parse_number::<u32>(
            &self.reconnect_attempts,
            EditorField::ReconnectAttempts,
            "Reconnect attempts",
            false,
            &mut errors,
        );
        let reconnect_delay = parse_number::<u64>(
            &self.reconnect_delay,
            EditorField::ReconnectDelay,
            "Reconnect delay",
            false,
            &mut errors,
        );
        let max_packet_size = parse_number::<u32>(
            &self.max_packet_size,
            EditorField::MaxPacketSize,
            "Maximum packet size",
            true,
            &mut errors,
        );
        let window_size = parse_number::<u32>(
            &self.window_size,
            EditorField::WindowSize,
            "Window size",
            true,
            &mut errors,
        );

        if !errors.is_empty() {
            return Err(EditorValidationErrors(errors));
        }

        let mut profile = self.template.clone();
        profile.metadata.name = name;
        profile.metadata.modified_at = Utc::now();
        profile.connection = ConnectionConfig {
            host,
            port: ssh_port.expect("validated SSH port"),
            user,
            auth_type: self.auth_type.clone(),
            key_path,
            password_storage: self.password_storage,
        };
        profile.forwarding = ForwardingConfig {
            forwarding_type: self.forwarding_type.clone(),
            local_port,
            remote_host,
            remote_port,
            bind_address,
        };
        profile.options = TunnelOptions {
            compression: self.compression,
            keepalive_interval: keepalive_interval.expect("validated keepalive interval"),
            auto_reconnect: self.auto_reconnect,
            reconnect_attempts: reconnect_attempts.expect("validated reconnect attempts"),
            reconnect_delay: reconnect_delay.expect("validated reconnect delay"),
            tcp_keepalive: self.tcp_keepalive,
            max_packet_size: max_packet_size.expect("validated maximum packet size"),
            window_size: window_size.expect("validated window size"),
        };
        Ok(profile)
    }
}

impl Default for ProfileEditorDraft {
    fn default() -> Self {
        Self::new(true)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorField {
    Name,
    Host,
    SshPort,
    User,
    KeyPath,
    LocalPort,
    BindAddress,
    RemoteHost,
    RemotePort,
    KeepaliveInterval,
    ReconnectAttempts,
    ReconnectDelay,
    MaxPacketSize,
    WindowSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorValidationError {
    pub field: EditorField,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorValidationErrors(pub Vec<EditorValidationError>);

impl fmt::Display for EditorValidationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(first) = self.0.first() {
            if self.0.len() == 1 {
                formatter.write_str(&first.message)
            } else {
                write!(
                    formatter,
                    "{} (and {} more)",
                    first.message,
                    self.0.len() - 1
                )
            }
        } else {
            formatter.write_str("The profile contains invalid values")
        }
    }
}

impl std::error::Error for EditorValidationErrors {}

fn required(
    value: &str,
    field: EditorField,
    label: &str,
    errors: &mut Vec<EditorValidationError>,
) -> String {
    let value = value.trim().to_string();
    if value.is_empty() {
        errors.push(EditorValidationError {
            field,
            message: format!("{label} cannot be empty"),
        });
    }
    value
}

fn parse_number<T>(
    value: &str,
    field: EditorField,
    label: &str,
    must_be_positive: bool,
    errors: &mut Vec<EditorValidationError>,
) -> Option<T>
where
    T: std::str::FromStr + Default + PartialEq,
{
    match value.trim().parse::<T>() {
        Ok(number) if !must_be_positive || number != T::default() => Some(number),
        Ok(_) => {
            errors.push(EditorValidationError {
                field,
                message: format!("{label} must be greater than zero"),
            });
            None
        }
        Err(_) => {
            errors.push(EditorValidationError {
                field,
                message: format!("{label} must be a valid number"),
            });
            None
        }
    }
}

fn expand_home_path(value: &str) -> PathBuf {
    if value == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(value));
    }
    if let Some(relative) = value.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(relative);
        }
    }
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_draft() -> ProfileEditorDraft {
        let mut draft = ProfileEditorDraft::new(true);
        draft.name = "Database".to_string();
        draft.host = "ssh.example.test".to_string();
        draft.user = "alice".to_string();
        draft.key_path = "/home/alice/.ssh/id_ed25519".to_string();
        draft.local_port = "5432".to_string();
        draft.remote_host = "database.internal".to_string();
        draft.remote_port = "5432".to_string();
        draft
    }

    #[test]
    fn secret_debug_output_is_always_redacted() {
        let secret = SecretValue::new("do-not-print-me");
        let update = CredentialUpdate::Store(secret);
        let rendered = format!("{update:?}");
        assert!(!rendered.contains("do-not-print-me"));
        assert!(rendered.contains("REDACTED"));
    }

    #[test]
    fn draft_reports_structured_field_errors() {
        let mut draft = valid_draft();
        draft.ssh_port = "not a port".to_string();
        draft.remote_host.clear();

        let errors = draft.build().unwrap_err();
        assert!(errors
            .0
            .iter()
            .any(|error| error.field == EditorField::SshPort));
        assert!(errors
            .0
            .iter()
            .any(|error| error.field == EditorField::RemoteHost));
    }

    #[test]
    fn changing_auth_mode_rebuilds_only_structured_fields() {
        let mut draft = valid_draft();
        draft.auth_type = AuthType::PasswordWith2FA;
        draft.key_path.clear();
        draft.password_storage = PasswordStorage::Client;

        let profile = draft.build().unwrap();
        assert_eq!(profile.connection.auth_type, AuthType::PasswordWith2FA);
        assert!(profile.connection.key_path.is_none());
        assert_eq!(profile.connection.password_storage, PasswordStorage::Client);
    }

    #[test]
    fn duplicate_gets_a_new_identity_and_never_copies_storage() {
        let mut source = valid_draft().build().unwrap();
        source.connection.password_storage = PasswordStorage::Client;
        let duplicate = ProfileEditorDraft::duplicate(&source, "Database copy".to_string(), true);

        assert_ne!(duplicate.profile_id(), source.metadata.id);
        assert_eq!(duplicate.name, "Database copy");
        assert_eq!(duplicate.password_storage, PasswordStorage::None);
    }

    #[test]
    fn unsupported_forwarding_type_is_preserved() {
        let mut source = valid_draft().build().unwrap();
        source.forwarding.forwarding_type = ForwardingType::Remote;
        let mut draft = ProfileEditorDraft::edit(source, true);
        draft.name = "Renamed".to_string();

        let rebuilt = draft.build().unwrap();
        assert_eq!(rebuilt.forwarding.forwarding_type, ForwardingType::Remote);
    }

    #[test]
    fn remote_daemon_key_paths_are_not_expanded_on_the_client() {
        let mut draft = ProfileEditorDraft::new(false);
        draft.name = "Remote".to_string();
        draft.host = "ssh.example.test".to_string();
        draft.user = "alice".to_string();
        draft.key_path = "~/.ssh/id_ed25519".to_string();
        draft.local_port = "8080".to_string();
        draft.remote_host = "localhost".to_string();
        draft.remote_port = "80".to_string();

        let profile = draft.build().unwrap();
        assert_eq!(
            profile.connection.key_path,
            Some(PathBuf::from("~/.ssh/id_ed25519"))
        );
    }
}
