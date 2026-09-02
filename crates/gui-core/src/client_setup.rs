// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Toolkit-neutral first-launch client configuration.
//!
//! Client setup happens before [`crate::AppRuntime`] is constructed. The types in this
//! module describe local file/configuration facts only; daemon-originating strings never
//! select setup behavior.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use ssh_tunnel_common::{is_loopback_address, is_valid_host, ConnectionMode, DaemonClientConfig};
use zeroize::Zeroizing;

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Why normal runtime startup cannot use the current `cli.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientSetupIssueKind {
    Missing,
    Unreadable,
    Malformed,
    Incomplete,
}

/// A local client-configuration problem.
///
/// `message` is display-only. Callers select behavior from [`Self::kind`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSetupIssue {
    pub kind: ClientSetupIssueKind,
    pub message: String,
}

/// Result of inspecting the client configuration before runtime startup.
pub enum ClientSetupDiscovery {
    Ready(DaemonClientConfig),
    SnippetAvailable,
    SetupRequired(ClientSetupIssue),
}

impl fmt::Debug for ClientSetupDiscovery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready(config) => formatter
                .debug_struct("Ready")
                .field("connection_mode", &config.connection_mode)
                .field("daemon_host", &config.daemon_host)
                .field("daemon_port", &config.daemon_port)
                .field("daemon_url", &config.daemon_url)
                .field("auth_token", &"[REDACTED]")
                .field("tls_cert_fingerprint", &config.tls_cert_fingerprint)
                .finish(),
            Self::SnippetAvailable => formatter.write_str("SnippetAvailable"),
            Self::SetupRequired(issue) => {
                formatter.debug_tuple("SetupRequired").field(issue).finish()
            }
        }
    }
}

/// One user-editable setup field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientSetupField {
    Host,
    Port,
    AuthenticationToken,
    TlsFingerprint,
}

/// Machine-readable reason a field was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientSetupValidationCode {
    Required,
    InvalidHost,
    NonLoopbackHttp,
    InvalidPort,
    InvalidFingerprint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientSetupValidationError {
    pub field: ClientSetupField,
    pub code: ClientSetupValidationCode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSetupValidationErrors(Vec<ClientSetupValidationError>);

impl ClientSetupValidationErrors {
    pub fn errors(&self) -> &[ClientSetupValidationError] {
        &self.0
    }

    pub fn contains(&self, field: ClientSetupField, code: ClientSetupValidationCode) -> bool {
        self.0
            .iter()
            .any(|error| error.field == field && error.code == code)
    }
}

impl fmt::Display for ClientSetupValidationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("client connection settings are incomplete or invalid")
    }
}

impl std::error::Error for ClientSetupValidationErrors {}

/// Redaction-safe editable client configuration.
pub struct ClientSetupDraft {
    pub connection_mode: ConnectionMode,
    pub daemon_host: String,
    pub daemon_port: String,
    pub daemon_url: String,
    auth_token: Zeroizing<String>,
    pub tls_cert_fingerprint: String,
    pub skip_ssh_setup_warning: bool,
}

impl fmt::Debug for ClientSetupDraft {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClientSetupDraft")
            .field("connection_mode", &self.connection_mode)
            .field("daemon_host", &self.daemon_host)
            .field("daemon_port", &self.daemon_port)
            .field("daemon_url", &self.daemon_url)
            .field("auth_token", &"[REDACTED]")
            .field("tls_cert_fingerprint", &self.tls_cert_fingerprint)
            .field("skip_ssh_setup_warning", &self.skip_ssh_setup_warning)
            .finish()
    }
}

impl Clone for ClientSetupDraft {
    fn clone(&self) -> Self {
        Self {
            connection_mode: self.connection_mode.clone(),
            daemon_host: self.daemon_host.clone(),
            daemon_port: self.daemon_port.clone(),
            daemon_url: self.daemon_url.clone(),
            auth_token: Zeroizing::new(self.auth_token.to_string()),
            tls_cert_fingerprint: self.tls_cert_fingerprint.clone(),
            skip_ssh_setup_warning: self.skip_ssh_setup_warning,
        }
    }
}

impl Default for ClientSetupDraft {
    fn default() -> Self {
        Self::from_config(DaemonClientConfig::default())
    }
}

impl ClientSetupDraft {
    pub fn from_config(config: DaemonClientConfig) -> Self {
        Self {
            connection_mode: config.connection_mode,
            daemon_host: config.daemon_host,
            daemon_port: config.daemon_port.to_string(),
            daemon_url: config.daemon_url,
            auth_token: Zeroizing::new(config.auth_token),
            tls_cert_fingerprint: config.tls_cert_fingerprint,
            skip_ssh_setup_warning: config.skip_ssh_setup_warning,
        }
    }

    pub fn authentication_token(&self) -> &str {
        self.auth_token.as_str()
    }

    pub fn set_authentication_token(&mut self, token: impl Into<String>) {
        self.auth_token = Zeroizing::new(token.into());
    }

    pub fn needs_network_host(&self) -> bool {
        matches!(
            self.connection_mode,
            ConnectionMode::Http | ConnectionMode::Https
        ) && self.daemon_host.trim().is_empty()
    }

    pub fn validate(&self) -> std::result::Result<DaemonClientConfig, ClientSetupValidationErrors> {
        let mut errors = Vec::new();
        let network_mode = matches!(
            self.connection_mode,
            ConnectionMode::Http | ConnectionMode::Https
        );
        let host = self.daemon_host.trim();

        if network_mode {
            if host.is_empty() {
                errors.push(ClientSetupValidationError {
                    field: ClientSetupField::Host,
                    code: ClientSetupValidationCode::Required,
                });
            } else if !is_valid_host(host) {
                errors.push(ClientSetupValidationError {
                    field: ClientSetupField::Host,
                    code: ClientSetupValidationCode::InvalidHost,
                });
            } else if matches!(self.connection_mode, ConnectionMode::Http)
                && !is_loopback_address(host)
            {
                errors.push(ClientSetupValidationError {
                    field: ClientSetupField::Host,
                    code: ClientSetupValidationCode::NonLoopbackHttp,
                });
            }
        }

        let port = if network_mode {
            match self.daemon_port.trim().parse::<u16>() {
                Ok(port) if port != 0 => Some(port),
                _ => {
                    errors.push(ClientSetupValidationError {
                        field: ClientSetupField::Port,
                        code: ClientSetupValidationCode::InvalidPort,
                    });
                    None
                }
            }
        } else {
            Some(3443)
        };

        if self.authentication_token().is_empty() {
            errors.push(ClientSetupValidationError {
                field: ClientSetupField::AuthenticationToken,
                code: ClientSetupValidationCode::Required,
            });
        }

        let fingerprint = self.tls_cert_fingerprint.trim();
        if matches!(self.connection_mode, ConnectionMode::Https) {
            if fingerprint.is_empty() {
                errors.push(ClientSetupValidationError {
                    field: ClientSetupField::TlsFingerprint,
                    code: ClientSetupValidationCode::Required,
                });
            } else if !is_valid_tls_fingerprint(fingerprint) {
                errors.push(ClientSetupValidationError {
                    field: ClientSetupField::TlsFingerprint,
                    code: ClientSetupValidationCode::InvalidFingerprint,
                });
            }
        }

        if !errors.is_empty() {
            return Err(ClientSetupValidationErrors(errors));
        }

        Ok(DaemonClientConfig {
            connection_mode: self.connection_mode.clone(),
            daemon_host: if network_mode {
                host.to_string()
            } else {
                "127.0.0.1".to_string()
            },
            daemon_port: port.expect("validated port is present"),
            daemon_url: if matches!(self.connection_mode, ConnectionMode::UnixSocket) {
                self.daemon_url.trim().to_string()
            } else {
                String::new()
            },
            auth_token: self.authentication_token().to_string(),
            tls_cert_fingerprint: if matches!(self.connection_mode, ConnectionMode::Https) {
                fingerprint.to_ascii_uppercase()
            } else {
                String::new()
            },
            skip_ssh_setup_warning: self.skip_ssh_setup_warning,
        })
    }
}

fn is_valid_tls_fingerprint(fingerprint: &str) -> bool {
    let octets: Vec<&str> = fingerprint.split(':').collect();
    octets.len() == 32
        && octets
            .iter()
            .all(|octet| octet.len() == 2 && octet.chars().all(|value| value.is_ascii_hexdigit()))
}

/// Client setup persistence with injectable paths for deterministic tests.
#[derive(Debug, Clone)]
pub struct ClientSetupRepository {
    config_path: PathBuf,
    snippet_path: PathBuf,
}

impl ClientSetupRepository {
    pub fn for_default_paths() -> Result<Self> {
        Ok(Self::new(
            crate::daemon::get_cli_config_path()?,
            ssh_tunnel_common::get_cli_config_snippet_path()?,
        ))
    }

    pub fn new(config_path: PathBuf, snippet_path: PathBuf) -> Self {
        Self {
            config_path,
            snippet_path,
        }
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn snippet_path(&self) -> &Path {
        &self.snippet_path
    }

    pub fn discover(&self) -> ClientSetupDiscovery {
        if !self.config_path.exists() {
            return if self.snippet_path.is_file() {
                ClientSetupDiscovery::SnippetAvailable
            } else {
                ClientSetupDiscovery::SetupRequired(ClientSetupIssue {
                    kind: ClientSetupIssueKind::Missing,
                    message: "No client connection configuration was found.".to_string(),
                })
            };
        }

        let contents = match fs::read_to_string(&self.config_path) {
            Ok(contents) => contents,
            Err(error) => {
                return ClientSetupDiscovery::SetupRequired(ClientSetupIssue {
                    kind: ClientSetupIssueKind::Unreadable,
                    message: format!(
                        "The existing client configuration could not be read: {error}"
                    ),
                });
            }
        };

        let config = match parse_config(&contents) {
            Ok(config) => config,
            Err(_) => {
                return ClientSetupDiscovery::SetupRequired(ClientSetupIssue {
                    kind: ClientSetupIssueKind::Malformed,
                    // Do not embed the parser error: TOML diagnostics can quote the source
                    // line, which may contain the daemon API token.
                    message: "The existing client configuration could not be parsed.".to_string(),
                });
            }
        };

        let draft_is_valid = ClientSetupDraft::from_config(config.clone())
            .validate()
            .is_ok();
        let common_contract_is_valid = ssh_tunnel_common::validate_client_config(&config).is_ok();
        if draft_is_valid && common_contract_is_valid {
            ClientSetupDiscovery::Ready(config)
        } else {
            ClientSetupDiscovery::SetupRequired(ClientSetupIssue {
                kind: ClientSetupIssueKind::Incomplete,
                message: "The existing client configuration is incomplete or invalid.".to_string(),
            })
        }
    }

    pub fn load_snippet(&self) -> Result<ClientSetupDraft> {
        let contents = fs::read_to_string(&self.snippet_path).with_context(|| {
            format!(
                "Failed to read configuration snippet {}",
                self.snippet_path.display()
            )
        })?;
        let config: DaemonClientConfig = toml::from_str(&contents).map_err(|_| {
            // TOML diagnostics can quote source lines, including an authentication token.
            anyhow::anyhow!(
                "Configuration snippet {} could not be parsed",
                self.snippet_path.display()
            )
        })?;
        Ok(ClientSetupDraft::from_config(config))
    }

    /// Load an existing configuration as an editable draft.
    ///
    /// This is useful for repairing a syntactically valid but incomplete configuration.
    pub fn load_existing(&self) -> Result<ClientSetupDraft> {
        let contents = fs::read_to_string(&self.config_path).with_context(|| {
            format!(
                "Failed to read client configuration {}",
                self.config_path.display()
            )
        })?;
        Ok(ClientSetupDraft::from_config(parse_config(&contents)?))
    }

    pub fn persist(&self, draft: &ClientSetupDraft) -> Result<DaemonClientConfig> {
        let config = draft.validate()?;
        self.persist_config(&config)?;
        Ok(config)
    }

    pub fn persist_config(&self, config: &DaemonClientConfig) -> Result<()> {
        // Keep compatibility callers behind the same validation boundary.
        ClientSetupDraft::from_config(config.clone()).validate()?;
        ssh_tunnel_common::validate_client_config(config)?;

        #[derive(serde::Serialize)]
        struct CliConfig<'a> {
            #[serde(flatten)]
            daemon_config: &'a DaemonClientConfig,
        }

        let contents = toml::to_string_pretty(&CliConfig {
            daemon_config: config,
        })
        .context("Failed to serialize client configuration")?;

        let parent = self
            .config_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Client configuration path has no parent directory"))?;
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create configuration directory {}",
                parent.display()
            )
        })?;

        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary_path = parent.join(format!(
            ".{}.{}.{}.tmp",
            self.config_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("cli.toml"),
            std::process::id(),
            sequence
        ));
        let temporary = TemporaryFile::new(temporary_path.clone());

        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        let mut file = options.open(&temporary_path).with_context(|| {
            format!(
                "Failed to create temporary client configuration {}",
                temporary_path.display()
            )
        })?;
        file.write_all(contents.as_bytes())
            .context("Failed to write client configuration")?;
        file.sync_all()
            .context("Failed to flush client configuration")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .context("Failed to secure client configuration")?;
        }
        drop(file);

        fs::rename(&temporary_path, &self.config_path).with_context(|| {
            format!(
                "Failed to replace client configuration {}",
                self.config_path.display()
            )
        })?;
        temporary.keep();

        Ok(())
    }
}

fn parse_config(contents: &str) -> Result<DaemonClientConfig> {
    #[derive(serde::Deserialize)]
    struct CliConfig {
        #[serde(flatten)]
        daemon_config: DaemonClientConfig,
    }

    Ok(toml::from_str::<CliConfig>(contents)?.daemon_config)
}

struct TemporaryFile {
    path: PathBuf,
    keep: std::cell::Cell<bool>,
}

impl TemporaryFile {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            keep: std::cell::Cell::new(false),
        }
    }

    fn keep(&self) {
        self.keep.set(true);
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        if !self.keep.get() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use tempfile::TempDir;

    use super::*;

    fn repository(temp: &TempDir) -> ClientSetupRepository {
        ClientSetupRepository::new(
            temp.path().join("config/cli.toml"),
            temp.path().join("config/cli-config.snippet"),
        )
    }

    fn complete_draft() -> ClientSetupDraft {
        let mut draft = ClientSetupDraft::default();
        draft.set_authentication_token("test-token");
        draft
    }

    #[test]
    fn discovery_distinguishes_missing_and_snippet_available() {
        let temp = TempDir::new().unwrap();
        let repository = repository(&temp);

        assert!(matches!(
            repository.discover(),
            ClientSetupDiscovery::SetupRequired(ClientSetupIssue {
                kind: ClientSetupIssueKind::Missing,
                ..
            })
        ));

        fs::create_dir_all(repository.snippet_path().parent().unwrap()).unwrap();
        fs::write(
            repository.snippet_path(),
            "connection_mode = \"unix-socket\"\nauth_token = \"test-token\"\n",
        )
        .unwrap();
        assert!(matches!(
            repository.discover(),
            ClientSetupDiscovery::SnippetAvailable
        ));
        assert!(repository.load_snippet().unwrap().validate().is_ok());
    }

    #[test]
    fn discovery_reports_malformed_incomplete_and_ready_config() {
        let temp = TempDir::new().unwrap();
        let repository = repository(&temp);
        fs::create_dir_all(repository.config_path().parent().unwrap()).unwrap();

        fs::write(repository.config_path(), "not = [valid").unwrap();
        assert!(matches!(
            repository.discover(),
            ClientSetupDiscovery::SetupRequired(ClientSetupIssue {
                kind: ClientSetupIssueKind::Malformed,
                ..
            })
        ));

        fs::write(
            repository.config_path(),
            "connection_mode = \"unix-socket\"\n",
        )
        .unwrap();
        assert!(matches!(
            repository.discover(),
            ClientSetupDiscovery::SetupRequired(ClientSetupIssue {
                kind: ClientSetupIssueKind::Incomplete,
                ..
            })
        ));

        repository.persist(&complete_draft()).unwrap();
        assert!(matches!(
            repository.discover(),
            ClientSetupDiscovery::Ready(_)
        ));
    }

    #[test]
    fn discovery_reports_unreadable_config_path() {
        let temp = TempDir::new().unwrap();
        let repository = repository(&temp);
        fs::create_dir_all(repository.config_path()).unwrap();

        assert!(matches!(
            repository.discover(),
            ClientSetupDiscovery::SetupRequired(ClientSetupIssue {
                kind: ClientSetupIssueKind::Unreadable,
                ..
            })
        ));
    }

    #[test]
    fn validation_uses_typed_mode_specific_errors() {
        let mut draft = ClientSetupDraft::default();
        let errors = draft.validate().unwrap_err();
        assert!(errors.contains(
            ClientSetupField::AuthenticationToken,
            ClientSetupValidationCode::Required
        ));
        assert!(!errors.errors().iter().any(|error| {
            matches!(
                error.field,
                ClientSetupField::Host | ClientSetupField::Port | ClientSetupField::TlsFingerprint
            )
        }));

        draft.connection_mode = ConnectionMode::Https;
        draft.daemon_host = "999.1.1.1".to_string();
        draft.daemon_port = "0".to_string();
        let errors = draft.validate().unwrap_err();
        assert!(errors.contains(
            ClientSetupField::Host,
            ClientSetupValidationCode::InvalidHost
        ));
        assert!(errors.contains(
            ClientSetupField::Port,
            ClientSetupValidationCode::InvalidPort
        ));
        assert!(errors.contains(
            ClientSetupField::TlsFingerprint,
            ClientSetupValidationCode::Required
        ));

        draft.connection_mode = ConnectionMode::Http;
        draft.daemon_host = "192.168.1.20".to_string();
        draft.daemon_port = "3443".to_string();
        let errors = draft.validate().unwrap_err();
        assert!(errors.contains(
            ClientSetupField::Host,
            ClientSetupValidationCode::NonLoopbackHttp
        ));

        draft.connection_mode = ConnectionMode::Https;
        draft.daemon_host = "daemon.example.com".to_string();
        draft.tls_cert_fingerprint = "sha256:not-a-certificate-fingerprint".to_string();
        let errors = draft.validate().unwrap_err();
        assert!(errors.contains(
            ClientSetupField::TlsFingerprint,
            ClientSetupValidationCode::InvalidFingerprint
        ));

        draft.tls_cert_fingerprint = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
        draft.set_authentication_token("test-token");
        let config = draft.validate().unwrap();
        assert_eq!(
            config.tls_cert_fingerprint,
            std::iter::repeat_n("AB", 32).collect::<Vec<_>>().join(":")
        );
    }

    #[test]
    fn snippet_with_bind_all_host_requires_structured_host_completion() {
        let temp = TempDir::new().unwrap();
        let repository = repository(&temp);
        fs::create_dir_all(repository.snippet_path().parent().unwrap()).unwrap();
        fs::write(
            repository.snippet_path(),
            "connection_mode = \"https\"\ndaemon_host = \"\"\ndaemon_port = 3443\nauth_token = \"test-token\"\ntls_cert_fingerprint = \"sha256:test\"\n",
        )
        .unwrap();

        let draft = repository.load_snippet().unwrap();
        assert!(draft.needs_network_host());
        assert!(draft
            .validate()
            .unwrap_err()
            .contains(ClientSetupField::Host, ClientSetupValidationCode::Required));
    }

    #[test]
    fn persistence_replaces_atomically_and_secures_the_target() {
        let temp = TempDir::new().unwrap();
        let repository = repository(&temp);
        let first = complete_draft();
        repository.persist(&first).unwrap();

        let mut second = first.clone();
        second.daemon_url = "/run/custom.sock".to_string();
        repository.persist(&second).unwrap();

        let contents = fs::read_to_string(repository.config_path()).unwrap();
        assert!(contents.contains("/run/custom.sock"));
        assert!(contents.contains("test-token"));
        let temporary_files = fs::read_dir(repository.config_path().parent().unwrap())
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(temporary_files, 0);

        #[cfg(unix)]
        assert_eq!(
            fs::metadata(repository.config_path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn debug_output_redacts_authentication_tokens() {
        let draft = complete_draft();
        let rendered = format!("{draft:?}");
        assert!(rendered.contains("[REDACTED]"));
        assert!(!rendered.contains("test-token"));

        let config = draft.validate().unwrap();
        let rendered = format!("{:?}", ClientSetupDiscovery::Ready(config));
        assert!(rendered.contains("[REDACTED]"));
        assert!(!rendered.contains("test-token"));
    }

    #[test]
    fn parser_errors_never_echo_authentication_tokens() {
        let temp = TempDir::new().unwrap();
        let repository = repository(&temp);
        fs::create_dir_all(repository.config_path().parent().unwrap()).unwrap();
        let malformed = "auth_token = \"should-never-be-echoed\" trailing\n";
        fs::write(repository.config_path(), malformed).unwrap();
        fs::write(repository.snippet_path(), malformed).unwrap();

        let ClientSetupDiscovery::SetupRequired(issue) = repository.discover() else {
            panic!("malformed configuration must require setup");
        };
        assert_eq!(issue.kind, ClientSetupIssueKind::Malformed);
        assert!(!issue.message.contains("should-never-be-echoed"));

        let snippet_error = repository.load_snippet().unwrap_err().to_string();
        assert!(!snippet_error.contains("should-never-be-echoed"));
    }
}
