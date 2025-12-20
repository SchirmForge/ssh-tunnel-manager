// Daemon monitor - SSE event listener with heartbeat monitoring

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::Deserialize;
use ssh_tunnel_common::{add_auth_header, create_daemon_client, AuthRequest, DaemonClientConfig};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration, Instant};
use uuid::Uuid;

use crate::notifications;
use crate::state::{TrayState, TunnelState};

/// Heartbeat timeout in seconds
const HEARTBEAT_TIMEOUT_SECS: u64 = 60;

/// Reconnect backoff in seconds
const RECONNECT_BACKOFF_SECS: u64 = 5;

/// Event from daemon SSE stream (matches daemon's OutgoingEvent)
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TunnelEvent {
    Starting { id: Uuid },
    Connected { id: Uuid },
    Disconnected { id: Uuid, reason: String },
    Error { id: Uuid, error: String },
    AuthRequired { id: Uuid, request: AuthRequest },
    Heartbeat { timestamp: DateTime<Utc> },
}

/// Start monitoring the daemon
pub async fn start_monitor(state: Arc<RwLock<TrayState>>) -> Result<()> {
    loop {
        // Get daemon config
        let config = {
            let state_lock = state.read().await;
            state_lock.daemon_config.clone()
        };

        // Try to connect and monitor
        match monitor_events(&config, state.clone()).await {
            Ok(_) => {
                tracing::info!("Event stream ended normally");
            }
            Err(e) => {
                tracing::warn!("Event stream error: {}", e);
            }
        }

        // Update status to disconnected
        {
            let mut state_lock = state.write().await;
            state_lock.last_heartbeat = None;
            state_lock.update_status();
        }

        // Wait before reconnecting
        sleep(Duration::from_secs(RECONNECT_BACKOFF_SECS)).await;
    }
}

/// Monitor events from daemon
async fn monitor_events(
    config: &DaemonClientConfig,
    state: Arc<RwLock<TrayState>>,
) -> Result<()> {
    let base_url = config.daemon_base_url()?;
    let url = format!("{}/api/events", base_url);

    // Create HTTP client
    let client = create_daemon_client(config)?;

    // Build request with auth
    let request = client.get(&url);
    let request = add_auth_header(request, config)?;

    // Send request and get response stream
    let response = request
        .send()
        .await
        .context("Failed to connect to event stream")?;

    if !response.status().is_success() {
        anyhow::bail!("Event stream request failed: {}", response.status());
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut last_heartbeat = Instant::now();

    tracing::info!("Connected to daemon event stream");

    loop {
        // Check for heartbeat timeout
        if last_heartbeat.elapsed() > Duration::from_secs(HEARTBEAT_TIMEOUT_SECS) {
            tracing::warn!("Heartbeat timeout, reconnecting...");
            break;
        }

        // Try to get next chunk with timeout
        match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(chunk_result)) => {
                match chunk_result {
                    Ok(bytes) => {
                        // Convert bytes to string
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            buffer.push_str(text);

                            // Process complete SSE messages
                            while let Some(pos) = buffer.find("\n\n") {
                                let message = buffer[..pos].to_string();
                                buffer = buffer[pos + 2..].to_string();

                                // Parse and handle SSE message
                                if let Some(event) = parse_sse_message(&message) {
                                    handle_event(event, state.clone()).await;
                                    last_heartbeat = Instant::now();
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error reading event stream: {}", e);
                        break;
                    }
                }
            }
            Ok(None) => {
                // Stream ended
                tracing::info!("Event stream ended");
                break;
            }
            Err(_) => {
                // Timeout, continue loop to check heartbeat
                continue;
            }
        }
    }

    Ok(())
}

/// Parse an SSE message into a TunnelEvent
fn parse_sse_message(message: &str) -> Option<TunnelEvent> {
    for line in message.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if let Ok(event) = serde_json::from_str::<TunnelEvent>(data) {
                return Some(event);
            } else {
                tracing::warn!("Failed to parse SSE event: {}", data);
            }
        }
    }
    None
}

/// Handle a tunnel event
async fn handle_event(event: TunnelEvent, state: Arc<RwLock<TrayState>>) {
    match event {
        TunnelEvent::Starting { id } => {
            tracing::info!("Tunnel {} starting", id);
        }
        TunnelEvent::Connected { id } => {
            tracing::info!("Tunnel {} connected", id);

            // Add to active tunnels
            let mut state_lock = state.write().await;
            if let Ok(profile) = ssh_tunnel_common::load_profile_by_id(&id) {
                state_lock.active_tunnels.insert(
                    id,
                    TunnelState {
                        profile_id: id,
                        profile_name: profile.metadata.name.clone(),
                        connected_at: Utc::now(),
                    },
                );
                state_lock.add_recent_profile(&profile);
            }
            state_lock.update_status();
        }
        TunnelEvent::Disconnected { id, reason } => {
            tracing::info!("Tunnel {} disconnected: {}", id, reason);

            // Get profile name before removing
            let profile_name = {
                let state_lock = state.read().await;
                state_lock
                    .active_tunnels
                    .get(&id)
                    .map(|t| t.profile_name.clone())
            };

            // Remove from active tunnels
            {
                let mut state_lock = state.write().await;
                state_lock.active_tunnels.remove(&id);
                state_lock.update_status();
            }

            // Show notification
            if let Some(name) = profile_name {
                notifications::show_disconnect_notification(&name, &reason, id);
            }
        }
        TunnelEvent::Error { id, error } => {
            tracing::error!("Tunnel {} error: {}", id, error);

            // Get profile name
            let profile_name = {
                let state_lock = state.read().await;
                state_lock
                    .active_tunnels
                    .get(&id)
                    .map(|t| t.profile_name.clone())
            };

            // Remove from active tunnels
            {
                let mut state_lock = state.write().await;
                state_lock.active_tunnels.remove(&id);
                state_lock.update_status();
            }

            // Show error notification
            if let Some(name) = profile_name {
                notifications::show_error_notification(&name, &error);
            }
        }
        TunnelEvent::AuthRequired { id, request } => {
            tracing::info!("Auth required for tunnel {}: {}", id, request.prompt);
            // Auth requests are handled by the GUI when user clicks
        }
        TunnelEvent::Heartbeat { timestamp } => {
            let mut state_lock = state.write().await;
            state_lock.last_heartbeat = Some(timestamp);
            state_lock.update_status();
        }
    }
}
