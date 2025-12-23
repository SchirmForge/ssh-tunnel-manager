// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// SSH Tunnel Manager - Tunnel Module
// Handles SSH connections and port forwarding using russh

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use russh::client::{self, AuthResult, Config, Handle, KeyboardInteractiveAuthResponse};
use russh::keys::{load_secret_key, PrivateKey, PrivateKeyWithHashAlg};
use tokio::io::copy_bidirectional;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
//use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use ssh_tunnel_common::{
    AuthRequest, AuthRequestType, ForwardingType, PasswordStorage, Profile, TunnelStatus,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const AUTH_RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);

/// Event sent when tunnel state changes (for future WebSocket notifications to GUI)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum TunnelEvent {
    Starting { id: Uuid },
    Connected { id: Uuid },
    Disconnected { id: Uuid, reason: String },
    Error { id: Uuid, error: String },
    AuthRequired { id: Uuid, request: AuthRequest },
}

/// Channel for sending auth responses to a waiting tunnel
pub type AuthResponseSender = oneshot::Sender<String>;

/// Pending authentication request
pub struct PendingAuth {
    pub request: AuthRequest,
    pub response_tx: AuthResponseSender,
}

/// State of an active tunnel
pub struct ActiveTunnel {
    pub profile: Profile,
    pub status: TunnelStatus,
    /// Channel to signal shutdown
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Pending authentication request, if any
    pub pending_auth: Option<PendingAuth>,
    join_handle: Option<tokio::task::JoinHandle<()>>,
}

// Manual Debug impl since PendingAuth contains oneshot channels
impl std::fmt::Debug for ActiveTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveTunnel")
            .field("profile", &self.profile.metadata.name)
            .field("status", &self.status)
            .field("has_pending_auth", &self.pending_auth.is_some())
            .finish()
    }
}

/// Manages all SSH tunnels
#[derive(Clone)]
pub struct TunnelManager {
    /// Active tunnels indexed by profile ID
    tunnels: Arc<RwLock<HashMap<Uuid, ActiveTunnel>>>,
    /// Event broadcaster
    event_tx: broadcast::Sender<TunnelEvent>,
    /// Path to known_hosts file (from daemon config)
    known_hosts_path: Arc<PathBuf>,
}

impl TunnelManager {
    pub fn new(known_hosts_path: PathBuf) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            tunnels: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            known_hosts_path: Arc::new(known_hosts_path),
        }
    }

    /// Subscribe to tunnel events
    pub fn subscribe(&self) -> broadcast::Receiver<TunnelEvent> {
        self.event_tx.subscribe()
    }

    /// Stop all active tunnels (best-effort)
    pub async fn stop_all(&self) {
        let ids: Vec<Uuid> = {
            let tunnels = self.tunnels.read().await;
            tunnels.keys().copied().collect()
        };

        for id in ids {
            if let Err(e) = self.stop(&id).await {
                warn!("Failed to stop tunnel {} during shutdown: {}", id, e);
            }
        }
    }

    /// Get the status of a tunnel
    pub async fn get_status(&self, id: &Uuid) -> Option<TunnelStatus> {
        let tunnels = self.tunnels.read().await;
        tunnels.get(id).map(|t| t.status.clone())
    }

    /// Get all active tunnel IDs and their statuses
    pub async fn list_active(&self) -> Vec<(Uuid, TunnelStatus)> {
        let tunnels = self.tunnels.read().await;
        tunnels
            .iter()
            .map(|(id, t)| (*id, t.status.clone()))
            .collect()
    }

    /// Get pending authentication request for a tunnel
    pub async fn get_pending_auth(&self, id: &Uuid) -> Option<AuthRequest> {
        let tunnels = self.tunnels.read().await;
        tunnels
            .get(id)
            .and_then(|t| t.pending_auth.as_ref())
            .map(|p| p.request.clone())
    }

    /// Submit authentication response for a tunnel
    pub async fn submit_auth(&self, id: &Uuid, response: String) -> Result<()> {
        let mut tunnels = self.tunnels.write().await;

        let tunnel = tunnels
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Tunnel not found"))?;

        let pending = tunnel
            .pending_auth
            .take()
            .ok_or_else(|| anyhow::anyhow!("No pending authentication request"))?;

        // Send the response to the waiting tunnel task
        pending
            .response_tx
            .send(response)
            .map_err(|_| anyhow::anyhow!("Tunnel task is no longer waiting for auth"))?;

        // Update status back to connecting
        tunnel.status = TunnelStatus::Connecting;

        Ok(())
    }

    /// Start a tunnel for the given profile
    pub async fn start(&self, profile: Profile) -> Result<()> {
        let id = profile.metadata.id;

        // Check if already running
        {
            let tunnels = self.tunnels.read().await;
            if let Some(tunnel) = tunnels.get(&id) {
                if tunnel.status.is_connected() {
                    anyhow::bail!("Tunnel {} is already connected", profile.metadata.name);
                }
                if tunnel.status.is_in_progress() {
                    anyhow::bail!(
                        "Tunnel {} connection is already in progress",
                        profile.metadata.name
                    );
                }
            }
        }

        // Validate profile
        profile
            .validate()
            .context("Invalid profile configuration")?;

        info!("Starting tunnel: {} ({})", profile.metadata.name, id);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);

        // Register tunnel as connecting
        {
            let mut tunnels = self.tunnels.write().await;
            tunnels.insert(
                id,
                ActiveTunnel {
                    profile: profile.clone(),
                    status: TunnelStatus::Connecting,
                    shutdown_tx: Some(shutdown_tx),
                    pending_auth: None,
                    join_handle: None,
                },
            );
        }

        if let Err(e) = self.event_tx.send(TunnelEvent::Starting { id }) {
            debug!("Failed to broadcast Starting event for {}: {}", id, e);
        }

        // Clone what we need for the background task
        let tunnels_for_task = self.tunnels.clone();
        let event_tx_for_task = self.event_tx.clone();
        let profile_for_task = profile.clone();
        let known_hosts_path_for_task = self.known_hosts_path.clone();

        // Spawn the tunnel task
        let handle = tokio::spawn(async move {
            match run_tunnel(
                profile_for_task.clone(),
                shutdown_rx,
                tunnels_for_task.clone(),
                event_tx_for_task.clone(),
                known_hosts_path_for_task,
            )
            .await
            {
                Ok(()) => {
                    info!("Tunnel {} stopped normally", profile_for_task.metadata.name);
                    if let Err(e) = event_tx_for_task.send(TunnelEvent::Disconnected {
                        id,
                        reason: "Stopped by user".to_string(),
                    }) {
                        debug!("Failed to broadcast Disconnected event for {}: {}", id, e);
                    }

                    // Update status to disconnected
                    let mut tunnels = tunnels_for_task.write().await;
                    if let Some(tunnel) = tunnels.get_mut(&id) {
                        tunnel.status = TunnelStatus::Disconnected;
                        tunnel.shutdown_tx = None;
                        tunnel.pending_auth = None;
                        tunnel.join_handle = None;
                    }
                }
                Err(e) => {
                    error!("Tunnel {} failed: {}", profile_for_task.metadata.name, e);

                    // Check if fail_tunnel() already handled this error
                    let already_failed = {
                        let tunnels = tunnels_for_task.read().await;
                        tunnels.get(&id)
                            .map(|t| matches!(t.status, TunnelStatus::Failed(_)))
                            .unwrap_or(false)
                    };

                    // Only emit error event if not already handled
                    if !already_failed {
                        if let Err(err) = event_tx_for_task.send(TunnelEvent::Error {
                            id,
                            error: e.to_string(),
                        }) {
                            debug!("Failed to broadcast Error event for {}: {}", id, err);
                        }
                    }

                    // Record the failure so clients polling status can read the reason
                    let mut tunnels = tunnels_for_task.write().await;
                    if let Some(tunnel) = tunnels.get_mut(&id) {
                        // Only update if not already marked as failed
                        if !matches!(tunnel.status, TunnelStatus::Failed(_)) {
                            tunnel.status = TunnelStatus::Failed(e.to_string());
                        }
                        tunnel.pending_auth = None;
                        tunnel.shutdown_tx = None;
                        tunnel.join_handle = None;
                    } else {
                        // If it was already removed, re-insert a minimal failed entry
                        tunnels.insert(
                            id,
                            ActiveTunnel {
                                profile: profile_for_task.clone(),
                                status: TunnelStatus::Failed(e.to_string()),
                                shutdown_tx: None,
                                pending_auth: None,
                                join_handle: None,
                            },
                        );
                    }
                }
            }
        });

        // Store the join handle back into the ActiveTunnel
        {
            let mut tunnels = self.tunnels.write().await;
            if let Some(tunnel) = tunnels.get_mut(&id) {
                tunnel.join_handle = Some(handle);
            }
        }

        Ok(())
    }

    /// Stop a running tunnel
    pub async fn stop(&self, id: &Uuid) -> Result<()> {
        let mut tunnels = self.tunnels.write().await;

        let tunnel = tunnels
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Tunnel not found"))?;

        match tunnel.status {
            TunnelStatus::Connecting | TunnelStatus::WaitingForAuth => {
                info!(
                    "Aborting connection for tunnel: {}",
                    tunnel.profile.metadata.name
                );

                // Try graceful shutdown first
                if let Some(tx) = tunnel.shutdown_tx.take() {
                    // Send shutdown signal - this should cause the tunnel task to exit gracefully
                    let _ = tx.send(()).await;
                }

                // Drop the pending auth sender - this will cause the oneshot receiver
                // to return Err, which will be caught as "Auth request was cancelled"
                tunnel.pending_auth = None;

                // Give the task a moment to respond to shutdown signal
                // If it doesn't stop within 100ms, abort it forcefully
                if let Some(mut handle) = tunnel.join_handle.take() {
                    match tokio::time::timeout(
                        tokio::time::Duration::from_millis(100),
                        &mut handle
                    ).await {
                        Ok(result) => {
                            // Task finished gracefully within timeout
                            if let Err(e) = result {
                                if e.is_cancelled() {
                                    debug!("Tunnel task was cancelled");
                                } else {
                                    debug!("Tunnel task panicked: {:?}", e);
                                }
                            }
                        }
                        Err(_) => {
                            // Timeout elapsed, task didn't finish gracefully
                            handle.abort();
                        }
                    }
                }

                tunnel.status = TunnelStatus::Disconnected;

                // Emit disconnected event
                if let Err(e) = self.event_tx.send(TunnelEvent::Disconnected {
                    id: *id,
                    reason: "Stopped during authentication".to_string(),
                }) {
                    debug!("Failed to broadcast Disconnected event for {}: {}", id, e);
                }
            }
            _ if tunnel.status.is_connected() => {
                info!("Stopping tunnel: {}", tunnel.profile.metadata.name);

                tunnel.status = TunnelStatus::Disconnecting;
                if let Some(tx) = tunnel.shutdown_tx.take() {
                    let _ = tx.send(()).await;
                }
            }
            _ => {
                anyhow::bail!("Tunnel is not active");
            }
        }

        Ok(())
    }
}

/// SSH client handler for russh with keyboard-interactive support
struct ClientHandler {
    /// Tunnel ID for this connection
    #[allow(dead_code)]
    tunnel_id: Uuid,
    /// Profile for this tunnel (needed for host/port in host key verification)
    profile: Profile,
    /// Auth context for prompting user
    auth_context: AuthContext,
    /// Path to known_hosts file (from daemon config)
    known_hosts_path: PathBuf,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        use crate::known_hosts::{calculate_fingerprint, KnownHosts, VerifyResult};

        let host = &self.profile.connection.host;
        let port = self.profile.connection.port;

        // Load known_hosts file from configured path
        let mut known_hosts = KnownHosts::load_from_pathbuf(self.known_hosts_path.clone(), false)
            .map_err(|e| russh::Error::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to load known_hosts: {}", e)
            )))?;

        // Verify the host key
        match known_hosts.verify(host, port, server_public_key) {
            VerifyResult::Trusted => {
                info!("Host key verified for {}:{}", host, port);
                Ok(true)
            }

            VerifyResult::Unknown => {
                // First connection - prompt user to verify
                let fingerprint = calculate_fingerprint(server_public_key);

                // Extract key type from SSH wire format
                use russh::keys::PublicKeyBase64;
                let key_bytes = server_public_key.public_key_bytes();
                let key_type = if key_bytes.len() >= 4 {
                    let len = u32::from_be_bytes([key_bytes[0], key_bytes[1], key_bytes[2], key_bytes[3]]) as usize;
                    if key_bytes.len() >= 4 + len {
                        String::from_utf8_lossy(&key_bytes[4..4 + len]).to_string()
                    } else {
                        "unknown".to_string()
                    }
                } else {
                    "unknown".to_string()
                };

                let prompt = format!(
                    "The authenticity of host '{}:{}' can't be established.\n\
                     {} key fingerprint is {}.\n\
                     Are you sure you want to continue connecting? (yes/no)",
                    host, port, key_type, fingerprint
                );

                // Request user confirmation
                let response = self.auth_context.request_input(
                    ssh_tunnel_common::types::AuthRequestType::HostKeyVerification,
                    &prompt,
                    false,  // not hidden
                ).await.map_err(|e| russh::Error::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Host key verification prompt failed: {}", e)
                )))?;

                // Check user response
                let response_lower = response.trim().to_lowercase();
                if response_lower == "yes" || response_lower == "y" {
                    // User accepted - add to known_hosts
                    known_hosts.add(host, port, server_public_key)
                        .map_err(|e| russh::Error::from(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to add host key: {}", e)
                        )))?;

                    known_hosts.save()
                        .map_err(|e| russh::Error::from(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to save known_hosts: {}", e)
                        )))?;

                    info!("Host key accepted and saved for {}:{}", host, port);
                    Ok(true)
                } else {
                    // User rejected
                    warn!("Host key rejected by user for {}:{}", host, port);
                    Ok(false)
                }
            }

            VerifyResult::Mismatch { expected_fingerprint, actual_fingerprint, line_number } => {
                // KEY MISMATCH - Possible MITM attack!
                error!("@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");
                error!("@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @");
                error!("@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");
                error!("IT IS POSSIBLE THAT SOMEONE IS DOING SOMETHING NASTY!");
                error!("Someone could be eavesdropping on you right now (man-in-the-middle attack)!");
                error!("It is also possible that the host key has just been changed.");
                error!("");
                error!("Host: {}:{}", host, port);
                error!("Expected fingerprint: {}", expected_fingerprint);
                error!("Actual fingerprint: {}", actual_fingerprint);
                error!("");
                error!("To remove the old host key, edit the known_hosts file:");
                error!("  {}", known_hosts.path().display());
                error!("Remove line {} from this file.", line_number);
                error!("@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");

                // Hard reject - do not allow connection
                Ok(false)
            }
        }
    }
}

/// Context for authentication operations
#[derive(Clone)]
struct AuthContext {
    tunnel_id: Uuid,
    auth_request_tx: mpsc::Sender<(AuthRequest, AuthResponseSender)>,
}

impl AuthContext {
    /// Request input from the user and wait for response
    async fn request_input(
        &self,
        auth_type: AuthRequestType,
        prompt: &str,
        hidden: bool,
    ) -> Result<String> {
        let (response_tx, response_rx) = oneshot::channel();

        let request = AuthRequest {
            tunnel_id: self.tunnel_id,
            auth_type,
            prompt: prompt.to_string(),
            hidden,
        };

        // Send request to be picked up by API
        self.auth_request_tx
            .send((request, response_tx))
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send auth request"))?;

        // Wait for response from CLI (with a timeout)
        match tokio::time::timeout(AUTH_RESPONSE_TIMEOUT, response_rx).await {
            // Got a response before timeout
            Ok(Ok(response)) => Ok(response),

            // The oneshot sender was dropped (e.g. CLI disappeared)
            Ok(Err(_canceled)) => Err(anyhow::anyhow!("Auth request was cancelled")),

            // Timeout elapsed
            Err(_elapsed) => Err(anyhow::anyhow!(format!(
                "Authentication prompt timed out after {:?}",
                AUTH_RESPONSE_TIMEOUT
            ))),
        }
    }
}

/// Establish SSH connection and authenticate
/// Returns the authenticated session handle for monitoring
async fn establish_connection(
    profile: &Profile,
    tunnels: &Arc<RwLock<HashMap<Uuid, ActiveTunnel>>>,
    event_tx: &broadcast::Sender<TunnelEvent>,
    known_hosts_path: Arc<PathBuf>,
) -> Result<Handle<ClientHandler>> {
    let id = profile.metadata.id;

    // --- SSH client configuration ---
    let mut cfg = Config::default();

    // Use window & packet size from profile options
    cfg.window_size = profile.options.window_size;
    cfg.maximum_packet_size = profile.options.max_packet_size;
    info!(
        "Window Size set to {} k-bytes / Max Packet Size set to {} k-bytes",
        cfg.window_size / 1024,
        cfg.maximum_packet_size / 1024
    );

    // disable nagle for lower latency
    cfg.nodelay = true;

    // (coudl be removed) keepalives to avoid idle connections dying silently
    cfg.keepalive_interval = Some(Duration::from_secs(30));
    cfg.keepalive_max = 3;

    let config = Arc::new(cfg);
    // end of ssh client/tunnel configuration

    // Track status
    {
        let mut ts = tunnels.write().await;
        if let Some(t) = ts.get_mut(&id) {
            t.status = TunnelStatus::Connecting;
        }
    } // Release lock immediately to avoid blocking auth_handler

    // Channel for auth requests (daemon -> CLI)
    let (auth_request_tx, mut auth_request_rx) =
        mpsc::channel::<(AuthRequest, AuthResponseSender)>(1);

    let auth_ctx = AuthContext {
        tunnel_id: id,
        auth_request_tx,
    };

    let handler = ClientHandler {
        tunnel_id: id,
        profile: profile.clone(),
        auth_context: auth_ctx.clone(),
        known_hosts_path: (*known_hosts_path).clone(),
    };

    // Spawn a task to handle auth requests and update tunnel state
    // IMPORTANT: This must be spawned BEFORE connecting, because check_server_key()
    // may be called during connection and needs someone listening on the channel
    let tunnels_for_auth = tunnels.clone();
    let event_tx_for_auth = event_tx.clone();

    let auth_handler = tokio::spawn(async move {
        while let Some((request, response_tx)) = auth_request_rx.recv().await {
            let tunnel_id = request.tunnel_id;
            info!("Auth request received: {:?}", request.auth_type);

            // Update tunnel state with pending auth
            {
                let mut tunnels = tunnels_for_auth.write().await;
                if let Some(tunnel) = tunnels.get_mut(&tunnel_id) {
                    tunnel.status = TunnelStatus::WaitingForAuth;
                    tunnel.pending_auth = Some(PendingAuth {
                        request: request.clone(),
                        response_tx,
                    });
                }
            }

            // Emit event to CLI/GUI
            if let Err(e) = event_tx_for_auth.send(TunnelEvent::AuthRequired {
                id: tunnel_id,
                request,
            }) {
                debug!("Failed to broadcast AuthRequired event for {}: {}", tunnel_id, e);
            }
        }
    });

    // Connect to SSH server
    let addr = ssh_tunnel_common::format_host_port(&profile.connection.host, profile.connection.port);
    info!("Connecting to SSH server: {}", addr);

    // session wrapped in a timeout
    let mut session = match tokio::time::timeout(
        CONNECT_TIMEOUT,
        client::connect(config.clone(), &addr, handler),
    )
    .await
    {
        Ok(Ok(sess)) => sess,
        Ok(Err(e)) => {
            let reason = format!("Failed to connect to {}: {}", addr, e);
            fail_tunnel(tunnels, event_tx, id, &reason).await?;
            anyhow::bail!(reason);
        }
        Err(_) => {
            let reason = format!(
                "Connection to {} timed out after {:?}",
                addr, CONNECT_TIMEOUT
            );
            fail_tunnel(tunnels, event_tx, id, &reason).await?;
            anyhow::bail!(reason);
        }
    };

    // Authenticate (this may trigger AuthRequired event)
    let authenticated = authenticate(&mut session, &profile, &auth_ctx).await?;
    if !authenticated {
        let reason = "Authentication failed".to_string();
        error!("{}", reason);
        if let Err(e) = event_tx.send(TunnelEvent::Error {
            id,
            error: reason.clone(),
        }) {
            debug!("Failed to broadcast Error event for {}: {}", id, e);
        }
        let mut ts = tunnels.write().await;
        if let Some(t) = ts.get_mut(&id) {
            t.status = TunnelStatus::Disconnected;
            t.pending_auth = None;
        }
        anyhow::bail!(reason);
    }

    // Stop the auth handler task
    drop(auth_ctx);
    auth_handler.abort();

    info!("SSH authentication successful");

    // Return the authenticated session for monitoring
    Ok(session)
}

/// Monitor an established SSH tunnel
/// Manages port forwarding, health monitoring, and lifecycle
async fn monitor_tunnel(
    session: Handle<ClientHandler>,
    profile: Profile,
    mut shutdown_rx: mpsc::Receiver<()>,
    tunnels: Arc<RwLock<HashMap<Uuid, ActiveTunnel>>>,
    event_tx: broadcast::Sender<TunnelEvent>,
) -> Result<()> {
    let id = profile.metadata.id;

    // Run port forwarding based on type (blocks until forwarding ends)
    // Note: The Connected event is sent AFTER successful port binding inside the forwarding task
    let forward_result = tokio::select! {
        // Shutdown signal received
        _ = shutdown_rx.recv() => {
            info!("Received shutdown signal for tunnel {}", id);
            Ok(())
        }

        // Run forwarding (blocks until session dies or error)
        result = async {
            match profile.forwarding.forwarding_type {
                ForwardingType::Local => {
                    run_local_forward_task(&session, &profile, tunnels.clone(), event_tx.clone()).await
                }
                ForwardingType::Remote => {
                    Err(anyhow::anyhow!("Remote forwarding not yet implemented"))
                }
                ForwardingType::Dynamic => {
                    Err(anyhow::anyhow!("Dynamic (SOCKS) forwarding not yet implemented"))
                }
            }
        } => result
    };

    // Graceful disconnect
    if let Err(e) = session
        .disconnect(russh::Disconnect::ByApplication, "", "en")
        .await
    {
        debug!("Failed to disconnect gracefully: {}", e);
    }

    // Return the forward result
    forward_result
}

/// Run the actual SSH tunnel (connects, authenticates, then monitors)
async fn run_tunnel(
    profile: Profile,
    mut shutdown_rx: mpsc::Receiver<()>,
    tunnels: Arc<RwLock<HashMap<Uuid, ActiveTunnel>>>,
    event_tx: broadcast::Sender<TunnelEvent>,
    known_hosts_path: Arc<PathBuf>,
) -> Result<()> {
    // Phase 1: Establish connection and authenticate
    // Use tokio::select to allow cancellation during connection/auth
    let session = tokio::select! {
        result = establish_connection(&profile, &tunnels, &event_tx, known_hosts_path) => {
            result?
        }
        _ = shutdown_rx.recv() => {
            info!("Received shutdown signal during connection for tunnel {}", profile.metadata.id);
            return Ok(()); // Exit gracefully
        }
    };

    // Phase 2: Monitor tunnel lifecycle
    monitor_tunnel(session, profile, shutdown_rx, tunnels, event_tx).await
}

// Failed tunnel text explanation
async fn fail_tunnel(
    tunnels: &Arc<RwLock<HashMap<Uuid, ActiveTunnel>>>,
    event_tx: &broadcast::Sender<TunnelEvent>,
    id: Uuid,
    reason: &str,
) -> Result<()> {
    error!("Tunnel {} failed: {}", id, reason);
    if let Err(e) = event_tx.send(TunnelEvent::Error {
        id,
        error: reason.to_string(),
    }) {
        debug!("Failed to broadcast Error event for {}: {}", id, e);
    }

    let mut ts = tunnels.write().await;
    if let Some(t) = ts.get_mut(&id) {
        t.status = TunnelStatus::Failed(reason.to_string());
        t.shutdown_tx = None;
        t.pending_auth = None;
    }
    Ok(())
}

/// Authenticate with the SSH server
/// Handles multi-step authentication where server may require multiple methods
/// (e.g., publickey + password, or publickey + keyboard-interactive for 2FA)
async fn authenticate(
    session: &mut Handle<ClientHandler>,
    profile: &Profile,
    auth_ctx: &AuthContext,
) -> Result<bool> {
    let user = &profile.connection.user;

    match profile.connection.auth_type {
        ssh_tunnel_common::AuthType::Key => {
            let key_path = profile
                .connection
                .key_path
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Key path not specified"))?;

            info!("Authenticating with key: {}", key_path.display());
            let key_result =
                authenticate_with_key(session, user, key_path, auth_ctx, profile).await?;

            if key_result {
                return Ok(true);
            }

            // Key auth returned false (partial_success=true)
            // Server accepted the key but wants additional auth
            info!("Key authentication partial success, checking for additional auth methods");

            // Try keyboard-interactive next (handles 2FA, additional passwords, etc.)
            info!("Attempting keyboard-interactive authentication");
            authenticate_keyboard_interactive(session, user, auth_ctx).await
        }
        ssh_tunnel_common::AuthType::Password => {
            info!("Authenticating with password");
            authenticate_with_password(session, user, auth_ctx, profile).await
        }
        ssh_tunnel_common::AuthType::PasswordWith2FA => {
            info!("Authenticating with password + 2FA (keyboard-interactive)");
            authenticate_keyboard_interactive(session, user, auth_ctx).await
        }
    }
}

/// Helper function to request passphrase from CLI and load key
async fn request_passphrase_and_load(
    key_path: &Path,
    auth_ctx: &AuthContext,
) -> Result<PrivateKey> {
    // This emits an AuthRequired event to CLI
    let passphrase = auth_ctx
        .request_input(
            AuthRequestType::KeyPassphrase,
            &format!("Enter passphrase for key '{}': ", key_path.display()),
            true,
        )
        .await?;

    load_secret_key(key_path, Some(&passphrase)).context(format!(
        "Failed to decrypt SSH key from {}",
        key_path.display()
    ))
}

/// Authenticate using an SSH key (with optional passphrase)
async fn authenticate_with_key(
    session: &mut Handle<ClientHandler>,
    user: &str,
    key_path: &Path,
    auth_ctx: &AuthContext,
    profile: &Profile,
) -> Result<bool> {
    // Try to load with stored passphrase first if available
    let key = if profile.connection.password_storage == PasswordStorage::Keychain {
        match crate::security::get_stored_password(&profile.metadata.id) {
            Ok(passphrase) => {
                info!("Using stored passphrase from keychain");
                match load_secret_key(key_path, Some(&passphrase)) {
                    Ok(k) => k,
                    Err(e) => {
                        warn!("Stored passphrase failed, requesting new one: {}", e);
                        // Fall through to interactive prompt
                        request_passphrase_and_load(key_path, auth_ctx).await?
                    }
                }
            }
            Err(e) => {
                warn!("Failed to retrieve stored passphrase: {}", e);
                request_passphrase_and_load(key_path, auth_ctx).await?
            }
        }
    } else {
        // Try without passphrase first, then prompt if needed
        match load_secret_key(key_path, None) {
            Ok(key) => key,
            Err(e) => {
                let err_str = e.to_string().to_lowercase();
                if err_str.contains("encrypted")
                    || err_str.contains("passphrase")
                    || err_str.contains("decrypt")
                {
                    info!("Key is encrypted, requesting passphrase");
                    request_passphrase_and_load(key_path, auth_ctx).await?
                } else {
                    return Err(e).context(format!(
                        "Failed to load SSH key from {}",
                        key_path.display()
                    ));
                }
            }
        }
    };

    // Prepare key with hash algorithm (for RSA); non-RSA keys will just ignore it
    let key_with_alg = PrivateKeyWithHashAlg::new(
        Arc::new(key),
        session.best_supported_rsa_hash().await?.flatten(),
    );

    // Try to authenticate
    let auth_result = session
        .authenticate_publickey(user, key_with_alg)
        .await
        .context("Public key authentication failed")?;

    match auth_result {
        AuthResult::Success => Ok(true),
        AuthResult::Failure {
            remaining_methods,
            partial_success,
        } => {
            // Build a helpful error message
            let methods: Vec<String> = remaining_methods
                .iter()
                .map(|m| {
                    let s: &str = m.into();
                    s.to_string()
                })
                .collect();

            if partial_success {
                // Server accepted the key but wants more authentication
                info!("Public key accepted, server requires additional authentication: {}", methods.join(", "));
                Ok(false) // Return false to trigger fallback auth methods
            } else {
                // Server rejected the key
                let methods_str = if methods.is_empty() {
                    "No authentication methods available".to_string()
                } else {
                    format!("Server requires: {}", methods.join(", "))
                };
                let error_msg = format!("Public key authentication rejected. {}", methods_str);
                error!("{}", error_msg);
                anyhow::bail!(error_msg)
            }
        }
    }
}

/// Authenticate using password only
async fn authenticate_with_password(
    session: &mut Handle<ClientHandler>,
    user: &str,
    auth_ctx: &AuthContext,
    profile: &Profile,
) -> Result<bool> {
    // Try stored password first if available
    let password = if profile.connection.password_storage == PasswordStorage::Keychain {
        match crate::security::get_stored_password(&profile.metadata.id) {
            Ok(pwd) => {
                info!("Using stored password from keychain");
                pwd
            }
            Err(e) => {
                warn!(
                    "Failed to retrieve stored password, requesting interactively: {}",
                    e
                );
                auth_ctx
                    .request_input(AuthRequestType::Password, "Enter SSH password: ", true)
                    .await?
            }
        }
    } else {
        // Request password interactively
        auth_ctx
            .request_input(AuthRequestType::Password, "Enter SSH password: ", true)
            .await?
    };

    // Try to authenticate
    let auth_result = session
        .authenticate_password(user, &password)
        .await
        .context("Password authentication failed")?;

    match auth_result {
        AuthResult::Success => Ok(true),
        AuthResult::Failure {
            remaining_methods,
            partial_success,
        } => {
            // Build a helpful error message
            let methods: Vec<String> = remaining_methods
                .iter()
                .map(|m| {
                    let s: &str = m.into();
                    s.to_string()
                })
                .collect();

            let methods_str = if methods.is_empty() {
                "No authentication methods available".to_string()
            } else {
                format!("Server requires: {}", methods.join(", "))
            };

            let error_msg = if partial_success {
                format!(
                    "Password authentication partially successful. {} to complete authentication",
                    methods_str
                )
            } else {
                format!("Password authentication rejected. {}", methods_str)
            };

            error!("{}", error_msg);
            anyhow::bail!(error_msg)
        }
    }
}

/// Authenticate using keyboard-interactive (for 2FA)
async fn authenticate_keyboard_interactive(
    session: &mut client::Handle<ClientHandler>,
    user: &str,
    auth_ctx: &AuthContext,
) -> Result<bool> {
    info!(
        "Attempting keyboard-interactive authentication for user: {}",
        user
    );

    // Start keyboard-interactive auth WITHOUT wrapping in an extra timeout.
    let mut response = session
        .authenticate_keyboard_interactive_start(user, None)
        .await
        .context("failed to start keyboard-interactive authentication")?;

    loop {
        match response {
            KeyboardInteractiveAuthResponse::Success => {
                info!("Keyboard-interactive authentication successful");
                return Ok(true);
            }

            // In your russh version, Failure has no fields
            KeyboardInteractiveAuthResponse::Failure {
                remaining_methods,
                partial_success,
            } => {
                // Build a helpful error message
                let methods: Vec<String> = remaining_methods
                    .iter()
                    .map(|m| {
                        let s: &str = m.into();
                        s.to_string()
                    })
                    .collect();

                let methods_str = if methods.is_empty() {
                    "No authentication methods available".to_string()
                } else {
                    format!("Server requires: {}", methods.join(", "))
                };

                let error_msg = if partial_success {
                    format!(
                        "Keyboard-interactive authentication partially successful. {} to complete authentication",
                        methods_str
                    )
                } else {
                    format!("Keyboard-interactive authentication rejected. {}", methods_str)
                };

                error!("{}", error_msg);
                anyhow::bail!(error_msg)
            }

            KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => {
                debug!(
                    "Keyboard-interactive: info request: name={:?}, instructions={:?}, prompts={:?}",
                    name, instructions, prompts
                );

                // Some servers send an info request with 0 prompts.
                // The correct reply in SSH is also 0 responses.
                if prompts.is_empty() {
                    debug!("Keyboard-interactive: empty prompts, sending zero responses");
                    response = session
                        .authenticate_keyboard_interactive_respond(Vec::new())
                        .await
                        .context("failed to send empty kbd-int response")?;
                    continue;
                }

                let mut answers = Vec::with_capacity(prompts.len());

                for prompt in &prompts {
                    let mut full_prompt = String::new();

                    if !name.trim().is_empty() {
                        full_prompt.push_str(&name);
                        full_prompt.push('\n');
                    }

                    if !instructions.trim().is_empty() {
                        full_prompt.push_str(&instructions);
                        full_prompt.push('\n');
                    }

                    full_prompt.push_str(&prompt.prompt);

                    // `echo == false` -> sensitive input (TOTP / password)
                    let answer = auth_ctx
                        .request_input(
                            AuthRequestType::TwoFactorCode, // semantic type, UI prompt is `full_prompt`
                            &full_prompt,
                            !prompt.echo,
                        )
                        .await
                        .context("failed to get keyboard-interactive input from client")?;

                    answers.push(answer);
                }

                // Send answers and wait for the next step (another InfoRequest or final Success/Failure).
                response = session
                    .authenticate_keyboard_interactive_respond(answers)
                    .await
                    .context("failed to send keyboard-interactive responses")?;
            }
        }
    }
}

/// Run local port forwarding task (session health aware)
/// Returns when the SSH session dies or encounters a fatal error
async fn run_local_forward_task(
    session: &Handle<ClientHandler>,
    profile: &Profile,
    tunnels: Arc<RwLock<HashMap<Uuid, ActiveTunnel>>>,
    event_tx: broadcast::Sender<TunnelEvent>,
) -> Result<()> {
    let id = profile.metadata.id;
    let local_port = profile
        .forwarding
        .local_port
        .ok_or_else(|| anyhow::anyhow!("Local port not specified"))?;
    let remote_host = profile
        .forwarding
        .remote_host
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Remote host not specified"))?;
    let remote_port = profile
        .forwarding
        .remote_port
        .ok_or_else(|| anyhow::anyhow!("Remote port not specified"))?;

    let bind_addr: SocketAddr = ssh_tunnel_common::format_host_port(&profile.forwarding.bind_address, local_port)
        .parse()
        .context("Invalid bind address")?;

    info!(
        "Starting local forward: {} -> {}:{}",
        bind_addr, remote_host, remote_port
    );

    // Bind local port
    let listener = match TcpListener::bind(bind_addr).await {
        Ok(l) => l,
        Err(e) => {
            // Detect permission errors specifically for privileged ports
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                return Err(anyhow::anyhow!(
                    "Permission denied binding to {}. Port {} is privileged (<=1024) and requires elevated permissions.\n\
                     Run the daemon with: sudo ssh-tunnel-daemon\n\
                     Or grant CAP_NET_BIND_SERVICE capability: sudo setcap cap_net_bind_service=+ep /path/to/ssh-tunnel-daemon",
                    bind_addr, local_port
                ));
            }
            return Err(anyhow::anyhow!("Failed to bind to {}: {}", bind_addr, e));
        }
    };

    info!("Listening on {}", bind_addr);

    // Port binding successful! Update status and broadcast Connected event
    {
        let mut tunnels = tunnels.write().await;
        if let Some(tunnel) = tunnels.get_mut(&id) {
            tunnel.status = TunnelStatus::Connected;
            tunnel.pending_auth = None;
        }
    }
    if let Err(e) = event_tx.send(TunnelEvent::Connected { id }) {
        debug!("Failed to broadcast Connected event for {}: {}", id, e);
    }

    // Track consecutive channel failures to detect session death
    let mut consecutive_failures = 0;
    const MAX_CONSECUTIVE_FAILURES: u32 = 3;

    loop {
        // Accept new connections
        let accept_result = listener.accept().await;

        match accept_result {
            Ok((stream, peer_addr)) => {
                debug!("Accepted connection from {}", peer_addr);

                // Open channel to remote
                let channel = match session.channel_open_direct_tcpip(
                    remote_host,
                    remote_port.into(),
                    &peer_addr.ip().to_string(),
                    peer_addr.port().into(),
                ).await {
                    Ok(ch) => {
                        // Reset failure counter on success
                        consecutive_failures = 0;
                        ch
                    }
                    Err(e) => {
                        consecutive_failures += 1;
                        error!(
                            "Failed to open channel ({}/{}): {}",
                            consecutive_failures, MAX_CONSECUTIVE_FAILURES, e
                        );

                        // If we've had too many consecutive failures, the session is likely dead
                        if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                            return Err(anyhow::anyhow!(
                                "SSH session appears dead after {} consecutive channel failures",
                                MAX_CONSECUTIVE_FAILURES
                            ));
                        }
                        continue;
                    }
                };

                // Spawn task to handle the connection
                tokio::spawn(async move {
                    if let Err(e) = handle_forward_connection(stream, channel).await {
                        debug!("Forward connection ended: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

/// Handle a single forwarded connection
async fn handle_forward_connection(
    mut tcp_stream: tokio::net::TcpStream,
    channel: russh::Channel<client::Msg>,
) -> Result<()> {
    /*     let (mut tcp_read, mut tcp_write) = tcp_stream.split();
     */
    // Turn SSH channel into a bidirectional stream
    let mut channel_stream = channel.into_stream();

    // Efficiently copy data in both directions until EOF / error
    let (_from_tcp, _from_ssh) = copy_bidirectional(&mut tcp_stream, &mut channel_stream).await?;

    debug!(
        "Forward connection closed: {} bytes from TCP, {} bytes from SSH",
        _from_tcp, _from_ssh
    );

    Ok(())
}

impl Default for TunnelManager {
    fn default() -> Self {
        use crate::known_hosts::KnownHosts;
        // Use default known_hosts path for test/default instances
        let known_hosts_path = KnownHosts::default_path()
            .unwrap_or_else(|_| PathBuf::from("known_hosts"));
        Self::new(known_hosts_path)
    }
}
