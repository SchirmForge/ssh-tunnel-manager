// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// SSH Tunnel Manager - Daemon Client Module
// Shared daemon connection logic for CLI and GUI

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::sse::TunnelEvent;
use crate::tls::{create_insecure_tls_config, create_pinned_tls_config};
use crate::{AuthRequest, TunnelStatus, Uuid};

/// Connection mode for client to daemon communication
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum ConnectionMode {
    /// Unix domain socket (local-only)
    #[default]
    UnixSocket,
    /// HTTP (testing/localhost only, no TLS)
    Http,
    /// HTTPS with TLS (network-ready, secure)
    Https,
}

/// Client configuration for connecting to daemon
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaemonClientConfig {
    /// Connection mode (UnixSocket, Http, or Https)
    #[serde(default)]
    pub connection_mode: ConnectionMode,

    /// Daemon host for HTTP/HTTPS modes (e.g., "127.0.0.1" or "192.168.1.100")
    #[serde(default = "default_daemon_host")]
    pub daemon_host: String,

    /// Daemon port for HTTP/HTTPS modes (e.g., 3443)
    #[serde(default = "default_daemon_port")]
    pub daemon_port: u16,

    /// Daemon URL for UnixSocket mode (socket path override, optional)
    #[serde(default)]
    pub daemon_url: String,

    /// Authentication token (if daemon requires auth)
    #[serde(default)]
    pub auth_token: String,

    /// TLS certificate fingerprint for HTTPS mode (optional, enables cert pinning)
    #[serde(default)]
    pub tls_cert_fingerprint: String,

    /// Skip SSH key setup warning for remote daemon connections
    /// When true, the warning dialog about copying SSH keys to remote daemon is not shown
    #[serde(default)]
    pub skip_ssh_setup_warning: bool,
}

fn default_daemon_host() -> String {
    "127.0.0.1".to_string()
}

fn default_daemon_port() -> u16 {
    3443
}

impl Default for DaemonClientConfig {
    fn default() -> Self {
        Self {
            connection_mode: ConnectionMode::default(),
            daemon_host: default_daemon_host(),
            daemon_port: default_daemon_port(),
            daemon_url: String::new(),
            auth_token: String::new(),
            tls_cert_fingerprint: String::new(),
            skip_ssh_setup_warning: false,
        }
    }
}

impl DaemonClientConfig {
    /// Get the daemon base URL based on connection mode
    /// Constructs the full URL with protocol (http:// or https://) from connection_mode
    pub fn daemon_base_url(&self) -> Result<String> {
        match self.connection_mode {
            ConnectionMode::UnixSocket => {
                // For Unix socket, we use a fake URL that reqwest understands
                Ok("http://daemon".to_string())
            }
            ConnectionMode::Http => {
                // Construct HTTP URL from daemon_host:daemon_port
                let host_port = crate::format_host_port(&self.daemon_host, self.daemon_port);
                Ok(format!("http://{}", host_port))
            }
            ConnectionMode::Https => {
                // Construct HTTPS URL from daemon_host:daemon_port
                let host_port = crate::format_host_port(&self.daemon_host, self.daemon_port);
                Ok(format!("https://{}", host_port))
            }
        }
    }

    /// Get the Unix socket path (for UnixSocket mode)
    ///
    /// An absolute path in the config wins. Otherwise the daemon's own locations are probed in
    /// priority order -- see [`crate::runtime_paths::client_socket_candidates`], which is
    /// shared with the daemon so the two cannot disagree about where the socket is.
    pub fn socket_path(&self) -> Result<PathBuf> {
        if self.connection_mode == ConnectionMode::UnixSocket {
            let candidate = self.daemon_url.trim();
            // If explicit path provided in config, use it
            if !candidate.is_empty()
                && (candidate.starts_with('/')
                    || candidate.starts_with("./")
                    || candidate.starts_with("../"))
            {
                return Ok(PathBuf::from(candidate));
            }
        }

        let candidates = crate::runtime_paths::client_socket_candidates();
        if let Some(existing) = candidates.iter().find(|path| path.exists()) {
            return Ok(existing.clone());
        }

        // Nothing there yet. Report the location a daemon started by this user would create,
        // so the error names the path that was actually expected.
        crate::runtime_paths::daemon_socket_path()
    }
}

/// Create an HTTP client configured to connect to the daemon
///
/// Check if daemon configuration needs IP address for connection
/// Returns true if daemon_host is empty for HTTP/HTTPS modes
pub fn config_needs_ip_address(config: &DaemonClientConfig) -> bool {
    matches!(
        config.connection_mode,
        ConnectionMode::Http | ConnectionMode::Https
    ) && config.daemon_host.is_empty()
}

/// Validate daemon client configuration completeness
/// Returns error if configuration is incomplete or invalid
pub fn validate_client_config(config: &DaemonClientConfig) -> Result<()> {
    // Check if IP address is needed but missing
    if config_needs_ip_address(config) {
        anyhow::bail!(
            "daemon_host is required for {} mode but is empty. \
             The daemon is configured to listen on all interfaces (0.0.0.0). \
             Please specify the actual IP address to connect to (e.g., 127.0.0.1 or 192.168.1.100)",
            match config.connection_mode {
                ConnectionMode::Http => "HTTP",
                ConnectionMode::Https => "HTTPS",
                _ => "this",
            }
        );
    }

    // Validate auth token is present (daemon requires auth by default)
    if config.auth_token.is_empty() {
        anyhow::bail!("Authentication token is required but is empty");
    }

    // For HTTPS, validate TLS fingerprint (recommended for security)
    if matches!(config.connection_mode, ConnectionMode::Https)
        && config.tls_cert_fingerprint.is_empty()
    {
        anyhow::bail!("TLS certificate fingerprint is required for HTTPS mode but is empty");
    }

    Ok(())
}

/// How long a streaming response may go silent before it is treated as dead.
///
/// The daemon heartbeats every 10 seconds (`heartbeat_stream` in `crates/daemon/src/api.rs`),
/// so this tolerates two missed beats. **If that interval changes, change this too** — a
/// read timeout shorter than the heartbeat would tear down healthy streams.
const STREAM_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// How long an ordinary request may take end to end.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Apply the connection-mode setup shared by every client we build.
///
/// Factored out so the request client and the streaming client cannot drift apart in how they
/// reach the daemon — only in how they time out.
fn configure_transport(
    mut builder: reqwest::ClientBuilder,
    config: &DaemonClientConfig,
) -> Result<reqwest::ClientBuilder> {
    match config.connection_mode {
        ConnectionMode::UnixSocket => {
            let socket_path = config.socket_path()?;
            builder = builder.unix_socket(socket_path);
        }
        ConnectionMode::Http => {
            // HTTP mode - no TLS
        }
        ConnectionMode::Https => {
            // HTTPS mode - configure TLS with optional certificate pinning
            if !config.tls_cert_fingerprint.is_empty() {
                // Certificate pinning enabled
                let tls_config = create_pinned_tls_config(config.tls_cert_fingerprint.clone())?;
                builder = builder.use_preconfigured_tls(tls_config);
            } else {
                // No pinning - use default system roots (accept any valid cert)
                let tls_config = create_insecure_tls_config()?;
                builder = builder.use_preconfigured_tls(tls_config);
            }
        }
    }
    Ok(builder)
}

/// A client for ordinary request/response calls to the daemon.
///
/// **Do not use this for the event stream** — see [`create_streaming_client`].
///
/// # Arguments
/// * `config` - Daemon client configuration
///
/// # Returns
/// Configured reqwest::Client ready to connect to daemon
pub fn create_daemon_client(config: &DaemonClientConfig) -> Result<Client> {
    configure_transport(Client::builder().timeout(REQUEST_TIMEOUT), config)?
        .build()
        .context("Failed to build daemon client")
}

/// A client for long-lived streaming responses, such as `/api/events`.
///
/// The difference from [`create_daemon_client`] is the timeout, and it matters more than it
/// looks. `reqwest`'s `timeout` is a **total** request timeout that includes reading the
/// response body, so applying it to Server-Sent Events cut every stream at exactly 30 seconds
/// no matter how much traffic flowed. Paired with a reconnect backoff that never reset, the
/// client spent half its life disconnected and silently missed daemon events — including
/// authentication prompts, which then expired with nothing shown to the user.
///
/// `read_timeout` is the right tool: it fires only when *nothing arrives* for that long,
/// which the heartbeat makes a genuine liveness signal.
pub fn create_streaming_client(config: &DaemonClientConfig) -> Result<Client> {
    configure_transport(Client::builder().read_timeout(STREAM_READ_TIMEOUT), config)?
        .build()
        .context("Failed to build daemon streaming client")
}

/// Add authentication header to request if configured
///
/// # Arguments
/// * `request` - The reqwest RequestBuilder to add auth to
/// * `config` - Daemon client configuration containing auth token
///
/// # Returns
/// RequestBuilder with auth header added (if token present)
pub fn add_auth_header(
    request: reqwest::RequestBuilder,
    config: &DaemonClientConfig,
) -> Result<reqwest::RequestBuilder> {
    if !config.auth_token.is_empty() {
        Ok(request.header("X-Tunnel-Token", &config.auth_token))
    } else {
        Ok(request)
    }
}

/// Get the path to the daemon-generated CLI config snippet
pub fn get_cli_config_snippet_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
    Ok(config_dir
        .join("ssh-tunnel-manager")
        .join("cli-config.snippet"))
}

/// Check if the daemon-generated CLI config snippet exists
pub fn cli_config_snippet_exists() -> bool {
    get_cli_config_snippet_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Result type for config validation
#[derive(Debug)]
pub enum ConfigValidationResult {
    /// Config is valid and ready to use
    Valid,
    /// Config file doesn't exist but snippet is available
    MissingConfigSnippetAvailable(PathBuf),
    /// Config file doesn't exist and no snippet available
    MissingConfigNoSnippet,
}

/// Validate daemon client configuration before attempting connection
///
/// This should be called BEFORE any daemon connection attempt to provide
/// clear guidance to users about configuration issues.
///
/// # Arguments
/// * `config_path` - Path to the CLI config file (e.g., ~/.config/ssh-tunnel-manager/cli.toml)
///
/// # Returns
/// ConfigValidationResult indicating the status
pub fn validate_daemon_config(config_path: &Path) -> ConfigValidationResult {
    if config_path.exists() {
        return ConfigValidationResult::Valid;
    }

    // Config doesn't exist - check if snippet is available
    if let Ok(snippet_path) = get_cli_config_snippet_path() {
        if snippet_path.exists() {
            return ConfigValidationResult::MissingConfigSnippetAvailable(snippet_path);
        }
    }

    ConfigValidationResult::MissingConfigNoSnippet
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DaemonClientConfig::default();
        assert_eq!(config.connection_mode, ConnectionMode::UnixSocket);
        assert_eq!(config.daemon_host, "127.0.0.1");
        assert_eq!(config.daemon_port, 3443);
    }

    #[test]
    fn test_daemon_base_url() {
        // Unix socket mode
        let config = DaemonClientConfig {
            connection_mode: ConnectionMode::UnixSocket,
            ..Default::default()
        };
        assert_eq!(config.daemon_base_url().unwrap(), "http://daemon");

        // HTTP mode
        let config = DaemonClientConfig {
            connection_mode: ConnectionMode::Http,
            daemon_host: "127.0.0.1".to_string(),
            daemon_port: 3443,
            ..Default::default()
        };
        assert_eq!(config.daemon_base_url().unwrap(), "http://127.0.0.1:3443");

        // HTTPS mode
        let config = DaemonClientConfig {
            connection_mode: ConnectionMode::Https,
            daemon_host: "example.com".to_string(),
            daemon_port: 3443,
            ..Default::default()
        };
        assert_eq!(
            config.daemon_base_url().unwrap(),
            "https://example.com:3443"
        );

        // IPv6 literals must be bracketed (regression guard for the v0.1.7 fix)
        let config = DaemonClientConfig {
            connection_mode: ConnectionMode::Https,
            daemon_host: "::1".to_string(),
            daemon_port: 3443,
            ..Default::default()
        };
        assert_eq!(config.daemon_base_url().unwrap(), "https://[::1]:3443");
    }

    #[test]
    fn test_add_auth_header() {
        let client = Client::new();

        // Without token
        let config = DaemonClientConfig::default();
        let request = client.get("http://test");
        let result = add_auth_header(request, &config);
        assert!(result.is_ok());

        // With token
        let config_with_auth = DaemonClientConfig {
            auth_token: "test-token-123".to_string(),
            ..Default::default()
        };
        let request = client.get("http://test");
        let result = add_auth_header(request, &config_with_auth);
        assert!(result.is_ok());
    }
}

// ============================================================================
// SSE-First Tunnel Control Flow
// ============================================================================

// TunnelEvent moved to crate::sse module
// Re-exported from lib.rs

/// Response from tunnel status endpoint
#[derive(Debug, Deserialize)]
pub struct TunnelStatusResponse {
    pub id: Uuid,
    pub status: TunnelStatus,
    pub pending_auth: Option<AuthRequest>,
}

/// Callback trait for handling tunnel events and authentication
pub trait TunnelEventHandler: Send {
    /// Called when authentication is required
    /// Should return the authentication response (password, passphrase, etc.)
    fn on_auth_required(&mut self, request: &AuthRequest) -> Result<String>;

    /// Called when tunnel successfully connects (optional)
    fn on_connected(&mut self) {}

    /// Called on any event for logging/status updates (optional)
    fn on_event(&mut self, _event: &TunnelEvent) {}
}

/// Start a tunnel with SSE-first flow and interactive authentication
///
/// This is the recommended way to start tunnels as it provides:
/// - Real-time status updates via SSE
/// - Interactive authentication handling
/// - Proper timeout and error handling
/// - Fallback to REST polling if SSE fails
///
/// # Arguments
/// * `client` - Configured reqwest Client for daemon communication
/// * `config` - Daemon client configuration (for base URL and auth)
/// * `tunnel_id` - UUID of the tunnel/profile to start
/// * `handler` - Event handler for auth prompts and status updates
///
/// # Returns
/// Ok(()) when tunnel is successfully connected
pub async fn start_tunnel_with_events<H: TunnelEventHandler>(
    client: &Client,
    config: &DaemonClientConfig,
    tunnel_id: Uuid,
    profile: &crate::Profile,
    handler: &mut H,
) -> Result<()> {
    use crate::{
        get_remote_key_setup_message, prepare_profile_for_remote, ProfileSourceMode,
        StartTunnelRequest,
    };

    // A credential the client holds itself, offered once before any human is asked.
    let mut stored = ClientHeldCredential::for_profile(profile, config);
    let mut answered = AnsweredRequests::default();

    let base_url = config.daemon_base_url()?;

    // Subscribe before starting, so the daemon cannot raise a prompt — or connect outright —
    // in the window between the start request and this client being ready to hear about it.
    //
    // This is the shared `EventListener`, not a second hand-rolled subscription. That copy
    // used the *request* client, whose total timeout cuts a stream at 30 seconds regardless of
    // traffic, so any prompt answered after that was delivered to nobody.
    let listener = crate::sse::EventListener::new(config.clone());
    let mut event_rx = listener
        .listen_ready()
        .await
        .context("Failed to establish SSE connection")?;

    // Determine if daemon is remote (HTTP/HTTPS) vs local (Unix socket)
    let is_remote_daemon = matches!(
        config.connection_mode,
        ConnectionMode::Http | ConnectionMode::Https
    );

    // Prepare the start tunnel request
    let (mode, profile_opt) = if is_remote_daemon {
        // Remote daemon - send profile via API
        let remote_profile = prepare_profile_for_remote(profile)
            .context("Failed to prepare profile for remote daemon")?;

        // Show SSH key warning if using key authentication
        if let Some(key_path) = &profile.connection.key_path {
            let daemon_host = match &config.connection_mode {
                ConnectionMode::Http | ConnectionMode::Https => {
                    // Use daemon_host from config
                    Some(config.daemon_host.as_str())
                }
                _ => None,
            };
            let warning_msg = get_remote_key_setup_message(key_path, daemon_host, None);
            eprintln!("\n{}\n", warning_msg);
        }

        (ProfileSourceMode::Hybrid, Some(remote_profile))
    } else {
        // Local daemon (Unix socket) - load from filesystem
        (ProfileSourceMode::Local, None)
    };

    let start_request = StartTunnelRequest {
        profile_id: tunnel_id.to_string(),
        mode,
        profile: profile_opt,
    };

    // Now send start request (SSE is ready to receive events)
    let url = format!("{}/api/tunnels/{}/start", base_url, tunnel_id);
    let resp = add_auth_header(client.post(&url), config)?
        .json(&start_request)
        .send()
        .await
        .context("Failed to send start request to daemon. Is the daemon running?")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to start tunnel: {} - {}", status, body);
    }

    // SSE-driven flow, with a periodic status poll as a backstop.
    //
    // The daemon re-emits *outstanding auth prompts* to every new subscriber, so a prompt can
    // no longer be lost in a reconnect gap. A `Connected` event still can: it is a one-off with
    // nothing to replay. Without this poll such a tunnel would connect fine and the caller
    // would sit here until `overall_timeout`, so the fallback stays.
    let idle_fallback = Duration::from_secs(15);
    let overall_timeout = Duration::from_secs(60);
    let idle_timer = tokio::time::sleep(idle_fallback);
    let overall_timer = tokio::time::sleep(overall_timeout);
    tokio::pin!(idle_timer);
    tokio::pin!(overall_timer);

    loop {
        tokio::select! {
            _ = &mut overall_timer => {
                anyhow::bail!("Timed out waiting for tunnel to connect");
            }
            _ = &mut idle_timer => {
                // Fallback: check status via REST
                if let Some(status) = fetch_tunnel_status(client, config, tunnel_id).await? {
                    match status.status {
                        TunnelStatus::Connected => {
                            handler.on_connected();
                            return Ok(());
                        }
                        TunnelStatus::WaitingForAuth => {
                            if let Some(auth_request) = status.pending_auth {
                                handle_auth_interactive(client, config, tunnel_id, &auth_request, handler, &mut stored, &mut answered)
                                    .await?;
                            }
                        }
                        TunnelStatus::Failed(reason) => anyhow::bail!("Tunnel failed: {reason}"),
                        TunnelStatus::Disconnected | TunnelStatus::NotConnected => {
                            anyhow::bail!("Tunnel is not active");
                        }
                        _ => {}
                    }
                }
                idle_timer.as_mut().reset(tokio::time::Instant::now() + idle_fallback);
            }
            maybe_event = event_rx.recv() => {
                let Some(ev) = maybe_event else {
                    // The listener reconnects on its own, so it only ends when it has given
                    // up for good. Reconcile once before failing.
                    if let Some(status) = fetch_tunnel_status(client, config, tunnel_id).await? {
                        if status.status == TunnelStatus::Connected {
                            handler.on_connected();
                            return Ok(());
                        }
                    }
                    anyhow::bail!("Event stream closed and tunnel status unknown");
                };

                // `/api/events` is a global broadcast, so skip other tunnels' events without
                // letting them hold off the idle fallback below.
                if !event_concerns(&ev, tunnel_id) {
                    continue;
                }

                handler.on_event(&ev);
                match ev {
                    TunnelEvent::Connected { .. } => {
                        handler.on_connected();
                        return Ok(());
                    }
                    TunnelEvent::Error { error, .. } => anyhow::bail!("Tunnel failed: {error}"),
                    TunnelEvent::Disconnected { reason, .. } => anyhow::bail!("Tunnel disconnected: {reason}"),
                    TunnelEvent::AuthRequired { request, .. } => {
                        handle_auth_interactive(client, config, tunnel_id, &request, handler, &mut stored, &mut answered)
                            .await?;
                    }
                    TunnelEvent::Starting { .. } | TunnelEvent::Heartbeat { .. } => {}
                }
                idle_timer.as_mut().reset(tokio::time::Instant::now() + idle_fallback);
            }
        }
    }
}

/// Whether a broadcast event concerns `tunnel_id`.
///
/// Heartbeats concern every subscriber; everything else only its own tunnel.
fn event_concerns(event: &TunnelEvent, tunnel_id: Uuid) -> bool {
    match event {
        TunnelEvent::Heartbeat { .. } => true,
        TunnelEvent::Starting { id }
        | TunnelEvent::Connected { id }
        | TunnelEvent::Disconnected { id, .. }
        | TunnelEvent::Error { id, .. }
        | TunnelEvent::AuthRequired { id, .. } => *id == tunnel_id,
    }
}

/// Fetch tunnel status once via REST API
async fn fetch_tunnel_status(
    client: &Client,
    config: &DaemonClientConfig,
    tunnel_id: Uuid,
) -> Result<Option<TunnelStatusResponse>> {
    let base_url = config.daemon_base_url()?;
    let status_url = format!("{}/api/tunnels/{}/status", base_url, tunnel_id);
    let status_resp = add_auth_header(client.get(&status_url), config)?
        .send()
        .await
        .context("Failed to get tunnel status")?;

    if status_resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    if !status_resp.status().is_success() {
        anyhow::bail!("Status check failed: {}", status_resp.status());
    }

    let status: TunnelStatusResponse = status_resp
        .json()
        .await
        .context("Failed to parse tunnel status")?;

    Ok(Some(status))
}

/// Answers auth prompts from the client's own credential store, so the daemon does not have
/// to reach a keychain it may not share.
///
/// The daemon is unchanged by this: it raises its usual prompt over SSE and waits. All that
/// differs is who answers — this, or a human. That is what makes `PasswordStorage::Client`
/// work identically for a local and a remote daemon.
/// Whether a prompt may be answered from a credential the *client* holds.
///
/// Two conditions, both load-bearing:
///
/// - the profile keeps its secret client-side, once the legacy `Keychain` value is resolved
///   against where the daemon actually runs. That resolution is the v0.3.0 fix: a credential
///   saved here but looked for there is why "save to keychain" silently did nothing against a
///   remote daemon; and
/// - the prompt asks for something *stable for the profile*. A TOTP code is valid for one time
///   step, keyboard-interactive text is server-supplied and must be shown verbatim, and a host
///   key is a trust decision — none of those may come out of storage.
///
/// This is the single copy of that policy. The CLI and the GUI each submit their own answer
/// over their own HTTP client, but they must not disagree about *whether* to answer.
pub fn client_credential_applies(
    storage: crate::PasswordStorage,
    daemon_is_local: bool,
    request_type: &crate::AuthRequestType,
) -> bool {
    storage.resolved(daemon_is_local).is_client_held()
        && matches!(
            request_type,
            crate::AuthRequestType::Password | crate::AuthRequestType::KeyPassphrase
        )
}

/// [`client_credential_applies`] for a whole profile.
pub fn profile_uses_client_credential(
    profile: &crate::Profile,
    daemon_is_local: bool,
    request_type: &crate::AuthRequestType,
) -> bool {
    client_credential_applies(
        profile.connection.password_storage,
        daemon_is_local,
        request_type,
    )
}

struct ClientHeldCredential {
    profile_id: Uuid,
    storage: crate::PasswordStorage,
    daemon_is_local: bool,
    /// Whether the stored secret has already been offered for this tunnel start.
    spent: bool,
}

/// Requests already answered during this tunnel start.
///
/// The daemon re-sends outstanding prompts to a newly connected subscriber, so the same
/// `AuthRequest` arrives again whenever the stream reconnects. A request id identifies the
/// *question*, not the delivery: answering one twice would prompt the user a second time and
/// then fail, because the daemon takes the pending request on first submit and rejects the
/// second with "Request ID mismatch".
#[derive(Default)]
struct AnsweredRequests {
    ids: std::collections::HashSet<Uuid>,
}

impl AnsweredRequests {
    /// Record `id` as answered; returns false if it already was.
    fn record(&mut self, id: Uuid) -> bool {
        self.ids.insert(id)
    }
}

impl ClientHeldCredential {
    fn for_profile(profile: &crate::Profile, config: &DaemonClientConfig) -> Self {
        Self {
            profile_id: profile.metadata.id,
            storage: profile.connection.password_storage,
            daemon_is_local: config.connection_mode == ConnectionMode::UnixSocket,
            spent: false,
        }
    }

    /// Whether this prompt is one a stored credential may answer.
    fn may_answer(&self, request: &AuthRequest) -> bool {
        !self.spent
            && client_credential_applies(self.storage, self.daemon_is_local, &request.auth_type)
    }

    /// A stored answer for this prompt, or `None` to let a human answer.
    fn answer(&mut self, request: &AuthRequest) -> Option<String> {
        self.answer_with(request, crate::keychain::get_password)
    }

    /// [`answer`](Self::answer) with the lookup injected, so the decision can be tested
    /// without a live credential store.
    fn answer_with<F>(&mut self, request: &AuthRequest, lookup: F) -> Option<String>
    where
        F: FnOnce(&Uuid) -> crate::error::Result<String>,
    {
        if !self.may_answer(request) {
            return None;
        }

        match lookup(&self.profile_id) {
            Ok(secret) => {
                // Offered once per tunnel start. The daemon re-prompts on a rejected
                // credential, and replaying a stale one would burn through the server's
                // MaxAuthTries without the user ever being asked -- the same trap the
                // daemon-side stored-password path had to avoid.
                self.spent = true;
                Some(secret)
            }
            Err(e) => {
                // Nothing saved, or the store is unreachable. Either way, ask the human.
                tracing::debug!("No client-held credential for this profile: {e}");
                None
            }
        }
    }
}

/// Handle authentication request interactively
async fn handle_auth_interactive<H: TunnelEventHandler>(
    client: &Client,
    config: &DaemonClientConfig,
    tunnel_id: Uuid,
    auth_request: &AuthRequest,
    handler: &mut H,
    stored: &mut ClientHeldCredential,
    answered: &mut AnsweredRequests,
) -> Result<()> {
    if !answered.record(auth_request.id) {
        // A repeat of a question already answered. Silently ignore it: re-prompting would
        // ask the user the same thing twice, and re-submitting would be rejected.
        tracing::debug!(
            "Ignoring repeated auth request {} for tunnel {tunnel_id}",
            auth_request.id
        );
        return Ok(());
    }

    let response = match stored.answer(auth_request) {
        Some(secret) => secret,
        None => handler.on_auth_required(auth_request)?,
    };

    let base_url = config.daemon_base_url()?;
    let auth_url = format!("{}/api/tunnels/{}/auth", base_url, tunnel_id);
    // Include the request_id so the daemon can pair this response with the pending prompt
    let payload = serde_json::json!({
        "request_id": auth_request.id,
        "response": response,
    });

    let auth_resp = add_auth_header(client.post(&auth_url).json(&payload), config)?
        .send()
        .await
        .context("Failed to submit authentication")?;

    if !auth_resp.status().is_success() {
        let body = auth_resp.text().await.unwrap_or_default();
        anyhow::bail!("Auth submission failed: {}", body);
    }

    Ok(())
}

/// Stop a tunnel (simple REST call)
///
/// # Arguments
/// * `client` - Configured reqwest Client for daemon communication
/// * `config` - Daemon client configuration (for base URL and auth)
/// * `tunnel_id` - UUID of the tunnel/profile to stop
///
/// # Returns
/// Ok(()) if stopped successfully or tunnel was not running
pub async fn stop_tunnel(
    client: &Client,
    config: &DaemonClientConfig,
    tunnel_id: Uuid,
) -> Result<()> {
    let base_url = config.daemon_base_url()?;
    let url = format!("{}/api/tunnels/{}/stop", base_url, tunnel_id);
    let resp = add_auth_header(client.post(&url), config)?
        .send()
        .await
        .context("Failed to send stop request to daemon")?;

    if resp.status().is_success() || resp.status() == reqwest::StatusCode::NOT_FOUND {
        Ok(())
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to stop tunnel: {} - {}", status, body)
    }
}

#[cfg(test)]
mod client_held_credential_tests {
    use super::*;

    // --- Client-held credentials -------------------------------------------------

    fn prompt(auth_type: crate::AuthRequestType) -> AuthRequest {
        AuthRequest {
            id: Uuid::new_v4(),
            tunnel_id: Uuid::new_v4(),
            auth_type,
            prompt: "test prompt".into(),
            hidden: true,
        }
    }

    /// `enabled` selects a storage value that is client-held (or not) against a local daemon,
    /// which is what the policy actually keys on.
    fn holder(enabled: bool) -> ClientHeldCredential {
        ClientHeldCredential {
            profile_id: Uuid::new_v4(),
            storage: if enabled {
                crate::PasswordStorage::Client
            } else {
                crate::PasswordStorage::None
            },
            daemon_is_local: true,
            spent: false,
        }
    }

    fn stored(_: &Uuid) -> crate::error::Result<String> {
        Ok("stored-secret".to_string())
    }

    fn nothing_stored(_: &Uuid) -> crate::error::Result<String> {
        Err(crate::Error::Keychain("no password stored".into()))
    }

    #[test]
    fn only_this_tunnels_events_are_acted_on_and_heartbeats_always_are() {
        let mine = Uuid::new_v4();
        let theirs = Uuid::new_v4();

        assert!(event_concerns(&TunnelEvent::Connected { id: mine }, mine));
        assert!(
            !event_concerns(&TunnelEvent::Connected { id: theirs }, mine),
            "`/api/events` is a global broadcast: another tunnel connecting must not be \
             mistaken for this one"
        );

        // The heartbeat carries no tunnel id and is what keeps the idle fallback from firing
        // on a healthy but quiet stream, so it has to reach every caller.
        assert!(event_concerns(
            &TunnelEvent::Heartbeat {
                timestamp: chrono::Utc::now()
            },
            mine
        ));
    }

    #[test]
    fn a_password_prompt_is_answered_from_the_client_store() {
        let mut h = holder(true);
        assert_eq!(
            h.answer_with(&prompt(crate::AuthRequestType::Password), stored),
            Some("stored-secret".to_string())
        );
    }

    #[test]
    fn a_key_passphrase_prompt_is_answered_from_the_client_store() {
        let mut h = holder(true);
        assert_eq!(
            h.answer_with(&prompt(crate::AuthRequestType::KeyPassphrase), stored),
            Some("stored-secret".to_string())
        );
    }

    /// The important one. The daemon re-prompts when a credential is rejected; replaying a
    /// stale stored value would exhaust the server's MaxAuthTries without the user ever being
    /// asked, which is precisely the bug fixed on the daemon side in v0.2.0.
    #[test]
    fn a_stored_credential_is_offered_once_and_then_the_human_is_asked() {
        let mut h = holder(true);
        assert!(h
            .answer_with(&prompt(crate::AuthRequestType::Password), stored)
            .is_some());
        assert!(
            h.answer_with(&prompt(crate::AuthRequestType::Password), stored)
                .is_none(),
            "a rejected stored credential must not be replayed"
        );
    }

    /// One-time codes cannot come from storage: a TOTP code is valid for one time step.
    #[test]
    fn two_factor_and_keyboard_interactive_prompts_always_reach_the_human() {
        for auth_type in [
            crate::AuthRequestType::TwoFactorCode,
            crate::AuthRequestType::KeyboardInteractive,
        ] {
            let mut h = holder(true);
            let described = format!("{auth_type:?}");
            assert!(
                h.answer_with(&prompt(auth_type), stored).is_none(),
                "{described} must not be answered from storage"
            );
        }
    }

    /// Trusting a host key is a judgement, not a secret.
    #[test]
    fn host_key_verification_always_reaches_the_human() {
        let mut h = holder(true);
        assert!(h
            .answer_with(&prompt(crate::AuthRequestType::HostKeyVerification), stored)
            .is_none());
    }

    #[test]
    fn other_storage_modes_do_not_answer_from_the_client_store() {
        let mut h = holder(false);
        assert!(h
            .answer_with(&prompt(crate::AuthRequestType::Password), stored)
            .is_none());
    }

    /// Nothing saved, or an unreachable store: fall through to the human rather than fail.
    #[test]
    fn a_repeated_request_id_is_recorded_only_once() {
        let mut answered = AnsweredRequests::default();
        let id = Uuid::new_v4();
        assert!(answered.record(id), "first delivery must be handled");
        assert!(
            !answered.record(id),
            "a repeat of the same question must be ignored, not answered again"
        );
    }

    #[test]
    fn distinct_requests_are_each_answered() {
        let mut answered = AnsweredRequests::default();
        assert!(answered.record(Uuid::new_v4()));
        assert!(answered.record(Uuid::new_v4()));
    }

    #[test]
    fn a_lookup_miss_falls_through_to_the_human_without_spending_the_attempt() {
        let mut h = holder(true);
        assert!(h
            .answer_with(&prompt(crate::AuthRequestType::Password), nothing_stored)
            .is_none());
        // A miss must not count as the one permitted attempt: if the store becomes readable
        // later in the same tunnel start, it should still be used.
        assert!(h
            .answer_with(&prompt(crate::AuthRequestType::Password), stored)
            .is_some());
    }
}
