// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// SSH Tunnel Manager - Daemon Client Module
// Shared daemon connection logic for CLI and GUI

use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::tls::{create_insecure_tls_config, create_pinned_tls_config};
use crate::{AuthRequest, AuthResponse, TunnelStatus, Uuid};
use crate::sse::TunnelEvent;

/// Connection mode for client to daemon communication
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ConnectionMode {
    /// Unix domain socket (local-only)
    UnixSocket,
    /// HTTP (testing/localhost only, no TLS)
    Http,
    /// HTTPS with TLS (network-ready, secure)
    Https,
}

impl Default for ConnectionMode {
    fn default() -> Self {
        ConnectionMode::UnixSocket
    }
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
    /// Checks multiple locations in priority order:
    /// 1. Explicit path in daemon_url (if absolute path)
    /// 2. User runtime directory (/run/user/<uid>/ssh-tunnel-manager/ssh-tunnel-manager.sock)
    /// 3. Legacy user runtime directory (/run/user/<uid>/ssh-tunnel-manager.sock)
    /// 4. System-wide location (/run/ssh-tunnel-manager/ssh-tunnel-manager.sock)
    pub fn socket_path(&self) -> Result<PathBuf> {
        if self.connection_mode == ConnectionMode::UnixSocket {
            let candidate = self.daemon_url.trim();
            // If explicit path provided in config, use it
            if !candidate.is_empty()
                && (candidate.starts_with('/') || candidate.starts_with("./") || candidate.starts_with("../"))
            {
                return Ok(PathBuf::from(candidate));
            }
        }

        // Try user runtime directory first (for user-mode daemon)
        if let Some(runtime_dir) = dirs::runtime_dir() {
            let socket_dir = if runtime_dir.file_name() == Some(OsStr::new("ssh-tunnel-manager")) {
                runtime_dir.clone()
            } else {
                runtime_dir.join("ssh-tunnel-manager")
            };
            let user_socket = socket_dir.join("ssh-tunnel-manager.sock");
            if user_socket.exists() {
                return Ok(user_socket);
            }

            // Backward compatibility: legacy path without subdirectory
            let legacy_socket = runtime_dir.join("ssh-tunnel-manager.sock");
            if legacy_socket.exists() {
                return Ok(legacy_socket);
            }
        }

        // Fall back to system-wide location (for system-mode daemon)
        let system_socket = PathBuf::from("/run/ssh-tunnel-manager/ssh-tunnel-manager.sock");
        if system_socket.exists() {
            return Ok(system_socket);
        }

        // If neither exists, default to user runtime directory (will be created by daemon)
        dirs::runtime_dir()
            .map(|runtime_dir| {
                if runtime_dir.file_name() == Some(OsStr::new("ssh-tunnel-manager")) {
                    runtime_dir.join("ssh-tunnel-manager.sock")
                } else {
                    runtime_dir
                        .join("ssh-tunnel-manager")
                        .join("ssh-tunnel-manager.sock")
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Could not determine runtime directory and no system socket found"))
    }
}

/// Create an HTTP client configured to connect to the daemon
///
/// Check if daemon configuration needs IP address for connection
/// Returns true if daemon_host is empty for HTTP/HTTPS modes
pub fn config_needs_ip_address(config: &DaemonClientConfig) -> bool {
    matches!(config.connection_mode, ConnectionMode::Http | ConnectionMode::Https)
        && config.daemon_host.is_empty()
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
                _ => "this"
            }
        );
    }

    // Validate auth token is present (daemon requires auth by default)
    if config.auth_token.is_empty() {
        anyhow::bail!("Authentication token is required but is empty");
    }

    // For HTTPS, validate TLS fingerprint (recommended for security)
    if matches!(config.connection_mode, ConnectionMode::Https)
        && config.tls_cert_fingerprint.is_empty() {
        anyhow::bail!("TLS certificate fingerprint is required for HTTPS mode but is empty");
    }

    Ok(())
}

/// # Arguments
/// * `config` - Daemon client configuration
///
/// # Returns
/// Configured reqwest::Client ready to connect to daemon
pub fn create_daemon_client(config: &DaemonClientConfig) -> Result<Client> {
    let mut client_builder = Client::builder().timeout(Duration::from_secs(30));

    // Configure client based on connection mode
    match config.connection_mode {
        ConnectionMode::UnixSocket => {
            let socket_path = config.socket_path()?;
            client_builder = client_builder.unix_socket(socket_path);
        }
        ConnectionMode::Http => {
            // HTTP mode - no TLS
        }
        ConnectionMode::Https => {
            // HTTPS mode - configure TLS with optional certificate pinning
            if !config.tls_cert_fingerprint.is_empty() {
                // Certificate pinning enabled
                let tls_config = create_pinned_tls_config(config.tls_cert_fingerprint.clone())?;
                client_builder = client_builder.use_preconfigured_tls(tls_config);
            } else {
                // No pinning - use default system roots (accept any valid cert)
                let tls_config = create_insecure_tls_config()?;
                client_builder = client_builder.use_preconfigured_tls(tls_config);
            }
        }
    }

    client_builder
        .build()
        .context("Failed to build daemon client")
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
pub fn validate_daemon_config(config_path: &PathBuf) -> ConfigValidationResult {
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
        let mut config = DaemonClientConfig::default();

        // Unix socket mode
        config.connection_mode = ConnectionMode::UnixSocket;
        assert_eq!(config.daemon_base_url().unwrap(), "http://daemon");

        // HTTP mode
        config.connection_mode = ConnectionMode::Http;
        config.daemon_host = "127.0.0.1".to_string();
        config.daemon_port = 3443;
        assert_eq!(
            config.daemon_base_url().unwrap(),
            "http://127.0.0.1:3443"
        );

        // HTTPS mode
        config.connection_mode = ConnectionMode::Https;
        config.daemon_host = "example.com".to_string();
        config.daemon_port = 3443;
        assert_eq!(
            config.daemon_base_url().unwrap(),
            "https://example.com:3443"
        );
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
        let mut config_with_auth = DaemonClientConfig::default();
        config_with_auth.auth_token = "test-token-123".to_string();
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
    use crate::{prepare_profile_for_remote, get_remote_key_setup_message, ProfileSourceMode, StartTunnelRequest};

    let base_url = config.daemon_base_url()?;

    // Subscribe to SSE events BEFORE sending start request
    // This ensures we don't miss any events that fire immediately after the tunnel starts
    let client_for_events = client.clone();
    let config_for_events = config.clone();
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (sse_ready_tx, mut sse_ready_rx) = tokio::sync::mpsc::channel::<Result<()>>(1);

    tokio::spawn(async move {
        let url = format!("{}/api/events", config_for_events.daemon_base_url().unwrap());
        let request = match add_auth_header(client_for_events.get(&url), &config_for_events) {
            Ok(req) => req,
            Err(e) => {
                let err_msg = e.to_string();
                let _ = sse_ready_tx.send(Err(anyhow::anyhow!("{}", err_msg))).await;
                let _ = event_tx.send(Err(e));
                return;
            }
        };

        let resp = match request.send().await {
            Ok(r) => r,
            Err(e) => {
                let err_msg = e.to_string();
                let _ = sse_ready_tx.send(Err(anyhow::anyhow!("{}", err_msg))).await;
                let _ = event_tx.send(Err(anyhow::anyhow!(e)));
                return;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let err_msg = if status == reqwest::StatusCode::UNAUTHORIZED {
                format!(
                    "Authentication failed: 401 Unauthorized\n\n\
                    The daemon requires authentication but no valid token was provided.\n\
                    \n\
                    To fix this:\n\
                    1. Check if the daemon has generated a CLI config snippet at:\n\
                       ~/.config/ssh-tunnel-manager/cli-config.snippet\n\
                    \n\
                    2. Copy it to your CLI config:\n\
                       cp ~/.config/ssh-tunnel-manager/cli-config.snippet ~/.config/ssh-tunnel-manager/cli.toml\n\
                    \n\
                    3. Or manually add the auth_token to ~/.config/ssh-tunnel-manager/cli.toml\n\
                    \n\
                    The daemon generates this snippet on first startup when authentication is enabled."
                )
            } else {
                format!("Daemon returned non-success status for events: {}", status)
            };
            let _ = sse_ready_tx.send(Err(anyhow::anyhow!("{}", err_msg))).await;
            let _ = event_tx.send(Err(anyhow::anyhow!("{}", err_msg)));
            return;
        }

        // SSE connection established - signal ready
        let _ = sse_ready_tx.send(Ok(())).await;

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let _ = event_tx.send(Err(anyhow::anyhow!(e)));
                    break;
                }
            };

            buffer.push_str(std::str::from_utf8(&chunk).unwrap_or(""));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim_end().to_string();
                buffer.drain(..=pos);

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                if let Some(rest) = line.strip_prefix("data:") {
                    let json_str = rest.trim();
                    if json_str.is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<TunnelEvent>(json_str) {
                        Ok(ev) => {
                            // Filter events for this tunnel (except heartbeats)
                            let should_forward = match &ev {
                                TunnelEvent::Heartbeat { .. } => true,
                                TunnelEvent::Starting { id }
                                | TunnelEvent::Connected { id }
                                | TunnelEvent::Disconnected { id, .. }
                                | TunnelEvent::Error { id, .. }
                                | TunnelEvent::AuthRequired { id, .. } => *id == tunnel_id,
                            };

                            if should_forward {
                                let _ = event_tx.send(Ok(ev));
                            }
                        }
                        Err(e) => {
                            let _ = event_tx.send(Err(anyhow::anyhow!(
                                "Failed to parse event JSON: {e} (line: {json_str})"
                            )));
                        }
                    }
                }
            }
        }
    });

    // Wait for SSE connection to be ready (with timeout)
    match tokio::time::timeout(Duration::from_secs(5), sse_ready_rx.recv()).await {
        Ok(Some(Ok(()))) => {
            // SSE connection established, proceed with start request
        }
        Ok(Some(Err(e))) => {
            anyhow::bail!("Failed to establish SSE connection: {}", e);
        }
        Ok(None) | Err(_) => {
            anyhow::bail!("Timed out waiting for SSE connection to establish");
        }
    }

    // Determine if daemon is remote (HTTP/HTTPS) vs local (Unix socket)
    let is_remote_daemon = matches!(config.connection_mode, ConnectionMode::Http | ConnectionMode::Https);

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

    // SSE-driven flow with fallback
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
                                handle_auth_interactive(client, config, tunnel_id, &auth_request, handler).await?;
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
                match maybe_event {
                    Some(Ok(ev)) => {
                        handler.on_event(&ev);
                        match ev {
                            TunnelEvent::Connected { .. } => {
                                handler.on_connected();
                                return Ok(());
                            }
                            TunnelEvent::Error { error, .. } => anyhow::bail!("Tunnel failed: {error}"),
                            TunnelEvent::Disconnected { reason, .. } => anyhow::bail!("Tunnel disconnected: {reason}"),
                            TunnelEvent::AuthRequired { request, .. } => {
                                handle_auth_interactive(client, config, tunnel_id, &request, handler).await?;
                            }
                            TunnelEvent::Starting { .. } | TunnelEvent::Heartbeat { .. } => {}
                        }
                    }
                    Some(Err(e)) => {
                        eprintln!("Event stream error: {e}");
                    }
                    None => {
                        // Stream ended; reconcile once, then fail
                        if let Some(status) = fetch_tunnel_status(client, config, tunnel_id).await? {
                            if status.status == TunnelStatus::Connected {
                                handler.on_connected();
                                return Ok(());
                            }
                        }
                        anyhow::bail!("Event stream closed and tunnel status unknown");
                    }
                }
                idle_timer.as_mut().reset(tokio::time::Instant::now() + idle_fallback);
            }
        }
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

/// Handle authentication request interactively
async fn handle_auth_interactive<H: TunnelEventHandler>(
    client: &Client,
    config: &DaemonClientConfig,
    tunnel_id: Uuid,
    auth_request: &AuthRequest,
    handler: &mut H,
) -> Result<()> {
    let response = handler.on_auth_required(auth_request)?;

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
