// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// SSH Tunnel Manager - REST API Module
// Handles HTTP API endpoints for tunnel control

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};

// added for /api/event management
use axum::response::sse::Event;
use axum::response::Sse;
use futures::{stream, StreamExt};
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;

use serde::Serialize;
use tracing::{error, info};
use uuid::Uuid;

use ssh_tunnel_common::{
    load_profile_by_id, AuthRequest, AuthResponse, ProfileSourceMode, StartTunnelRequest,
    TunnelStatus,
};
use chrono::{DateTime, Utc};
use std::time::{Duration, SystemTime};

use crate::config::DaemonConfig;
use crate::tunnel::{TunnelEvent, TunnelManager};

/// Shared application state
pub struct AppState {
    pub tunnel_manager: TunnelManager,
    pub shutdown_tx: tokio::sync::broadcast::Sender<()>,
    pub started_at: Arc<tokio::sync::RwLock<SystemTime>>,
    pub config: Arc<DaemonConfig>,
}

/// API error response
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

/// API success response
#[derive(Serialize)]
struct SuccessResponse {
    message: String,
}

/// Tunnel status response
#[derive(Serialize)]
struct TunnelStatusResponse {
    id: Uuid,
    status: TunnelStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pending_auth: Option<AuthRequest>,
}

/// List of active tunnels
#[derive(Serialize)]
struct TunnelsListResponse {
    tunnels: Vec<TunnelStatusResponse>,
}

// Event type
#[derive(Debug, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutgoingEvent {
    Starting { id: Uuid },
    Connected { id: Uuid },
    Disconnected { id: Uuid, reason: String },
    Error { id: Uuid, error: String },
    AuthRequired { id: Uuid, request: AuthRequest },
    Heartbeat { timestamp: DateTime<Utc> },
}

/// Create the API router
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/daemon/info", get(get_daemon_info))
        .route("/api/daemon/shutdown", post(shutdown_daemon))
        .route("/api/tunnels", get(list_tunnels))
        .route("/api/tunnels/:id/start", post(start_tunnel))
        .route("/api/tunnels/:id/stop", post(stop_tunnel))
        .route("/api/tunnels/:id/status", get(tunnel_status))
        .route("/api/tunnels/:id/auth", get(get_pending_auth))
        .route("/api/tunnels/:id/auth", post(submit_auth))
        .route("/api/events", get(event_stream))
        .with_state(state)
}

/// Health check endpoint
async fn health() -> &'static str {
    "OK"
}

/// List all active tunnels
async fn list_tunnels(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tunnels = state.tunnel_manager.list_active().await;

    let mut response_tunnels = Vec::new();
    for (id, status) in tunnels {
        let pending_auth = state.tunnel_manager.get_pending_auth(&id).await;
        response_tunnels.push(TunnelStatusResponse {
            id,
            status,
            pending_auth,
        });
    }

    let response = TunnelsListResponse {
        tunnels: response_tunnels,
    };

    Json(response)
}

/// Start a tunnel
async fn start_tunnel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(request): Json<StartTunnelRequest>,
) -> impl IntoResponse {
    info!("API: Start tunnel request for {} (mode: {:?})", id, request.mode);

    // Validate that the profile_id in the request matches the URL path
    if request.profile_id != id.to_string() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "Profile ID mismatch: URL has {} but request has {}",
                    id, request.profile_id
                ),
            }),
        )
            .into_response();
    }

    // Get the profile based on the source mode
    let profile = match request.mode {
        ProfileSourceMode::Local => {
            // Load from daemon's filesystem
            match load_profile_by_id(&id) {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to load profile {} from filesystem: {}", id, e);
                    return (
                        StatusCode::NOT_FOUND,
                        Json(ErrorResponse {
                            error: format!("Profile not found on daemon filesystem: {}", e),
                        }),
                    )
                        .into_response();
                }
            }
        }
        ProfileSourceMode::Hybrid => {
            // Use profile from request
            match request.profile {
                Some(p) => {
                    // Validate SSH key exists if specified
                    if let Some(key_path) = &p.connection.key_path {
                        // Expand ~ to home directory
                        let home_dir = dirs::home_dir().unwrap_or_else(|| "/root".into());
                        let ssh_dir = home_dir.join(".ssh");
                        let full_key_path = ssh_dir.join(key_path);

                        if !full_key_path.exists() {
                            let key_filename = key_path.display();
                            let error_msg = format!(
                                "SSH key not found on daemon: ~/.ssh/{}\n\n\
                                To copy your SSH key to the daemon:\n\
                                1. Copy the private key:\n   \
                                   scp <local-key-path> <daemon-host>:~/.ssh/{}\n\n\
                                2. Set correct permissions:\n   \
                                   ssh <daemon-host> chmod 600 ~/.ssh/{}",
                                key_filename, key_filename, key_filename
                            );
                            error!("{}", error_msg);
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(ErrorResponse { error: error_msg }),
                            )
                                .into_response();
                        }
                    }
                    p
                }
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: "Hybrid mode requires profile data in request".to_string(),
                        }),
                    )
                        .into_response();
                }
            }
        }
        ProfileSourceMode::Remote => {
            // Not yet implemented
            return (
                StatusCode::NOT_IMPLEMENTED,
                Json(ErrorResponse {
                    error: "Remote mode not yet implemented".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Start the tunnel
    match state.tunnel_manager.start(profile).await {
        Ok(()) => {
            info!("Tunnel {} start initiated", id);
            (
                StatusCode::ACCEPTED,
                Json(SuccessResponse {
                    message: format!("Tunnel {} starting", id),
                }),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to start tunnel {}: {}", id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// Stop a tunnel
async fn stop_tunnel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    info!("API: Stop tunnel request for {}", id);

    match state.tunnel_manager.stop(&id).await {
        Ok(()) => {
            info!("Tunnel {} stop initiated", id);
            (
                StatusCode::OK,
                Json(SuccessResponse {
                    message: format!("Tunnel {} stopping", id),
                }),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to stop tunnel {}: {}", id, e);
            // Map common tunnel lifecycle errors to client-friendly status codes
            let msg = e.to_string();
            let status = if msg.contains("not active") || msg.contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            (status, Json(ErrorResponse { error: msg })).into_response()
        }
    }
}

/// Get tunnel status
async fn tunnel_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.tunnel_manager.get_status(&id).await {
        Some(status) => {
            let pending_auth = state.tunnel_manager.get_pending_auth(&id).await;
            (
                StatusCode::OK,
                Json(TunnelStatusResponse {
                    id,
                    status,
                    pending_auth,
                }),
            )
                .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Tunnel {} not found or not active", id),
            }),
        )
            .into_response(),
    }
}

/// Get pending authentication request for a tunnel
async fn get_pending_auth(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.tunnel_manager.get_pending_auth(&id).await {
        Some(auth_request) => (StatusCode::OK, Json(auth_request)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "No pending authentication request".to_string(),
            }),
        )
            .into_response(),
    }
}

/// Submit authentication response
async fn submit_auth(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(auth_response): Json<AuthResponse>,
) -> impl IntoResponse {
    info!("API: Auth response received for tunnel {}", id);

    // Verify the tunnel ID matches
    if auth_response.tunnel_id != id {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Tunnel ID in request body doesn't match URL".to_string(),
            }),
        )
            .into_response();
    }

    match state
        .tunnel_manager
        .submit_auth(&id, auth_response.response)
        .await
    {
        Ok(()) => {
            info!("Auth response submitted for tunnel {}", id);
            (
                StatusCode::OK,
                Json(SuccessResponse {
                    message: "Authentication response submitted".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to submit auth for tunnel {}: {}", id, e);
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// GET /api/events  → SSE stream of tunnel events
pub async fn event_stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    // subscribe to the broadcast channel
    let rx = state.tunnel_manager.subscribe();
    let mut shutdown_rx = state.shutdown_tx.subscribe();

    // Broadcast events from tunnel manager
    let tunnel_events = BroadcastStream::new(rx).filter_map(|msg| async move {
        match msg {
            Ok(ev) => {
                let outgoing = match ev {
                    TunnelEvent::Starting { id } => OutgoingEvent::Starting { id },
                    TunnelEvent::Connected { id } => OutgoingEvent::Connected { id },
                    TunnelEvent::Disconnected { id, reason } => {
                        OutgoingEvent::Disconnected { id, reason }
                    }
                    TunnelEvent::Error { id, error } => OutgoingEvent::Error { id, error },
                    TunnelEvent::AuthRequired { id, request } => {
                        OutgoingEvent::AuthRequired { id, request }
                    }
                };

                let json = match serde_json::to_string(&outgoing) {
                    Ok(j) => j,
                    Err(e) => {
                        tracing::error!("Failed to serialize OutgoingEvent: {e}");
                        return None;
                    }
                };

                Some(Ok(Event::default().data(json)))
            }
            Err(lagged) => {
                // We lagged behind in the broadcast channel
                // This happens when events are broadcast faster than this client can consume them
                // Continue processing - the client will catch up with future events
                tracing::debug!("Event stream lagged: {:?}, continuing", lagged);
                None
            }
        }
    });

    // Heartbeat stream to keep connections warm and allow clients to detect liveness
    let heartbeat_stream = heartbeat_stream();

    // Merge tunnel events and heartbeats
    let merged = stream::select(tunnel_events, heartbeat_stream);

    // Take events until shutdown signal is received
    let shutdown_aware = merged.take_until(async move {
        let _ = shutdown_rx.recv().await;
    });

    Sse::new(shutdown_aware)
}

fn heartbeat_stream(
) -> impl futures::Stream<Item = Result<Event, Infallible>> + Send + Sync + 'static {
    tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(heartbeat_interval()))
        .map(|_| {
            Ok(Event::default().data(heartbeat_payload()))
        })
}

fn heartbeat_payload() -> String {
    match serde_json::to_string(&OutgoingEvent::Heartbeat { timestamp: Utc::now() }) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to serialize heartbeat: {e}");
            "{}".to_string()
        }
    }
}

#[cfg(not(test))]
fn heartbeat_interval() -> Duration {
    Duration::from_secs(10)
}

#[cfg(test)]
fn heartbeat_interval() -> Duration {
    Duration::from_millis(100)
}

/// Get daemon information (version, config, uptime, etc.)
async fn get_daemon_info(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    use std::time::UNIX_EPOCH;
    use ssh_tunnel_common::DaemonInfo;

    // Compute uptime
    let started = *state.started_at.read().await;
    let uptime = SystemTime::now()
        .duration_since(started)
        .unwrap_or_default()
        .as_secs();

    // Format started_at as ISO 8601
    let started_at_timestamp = started
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let started_at_iso = chrono::DateTime::from_timestamp(started_at_timestamp as i64, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string());

    // Count active tunnels
    let tunnels = state.tunnel_manager.list_active().await;
    let active_count = tunnels.len();

    // Get config details
    let config = &state.config;

    // Determine listener mode string
    let listener_mode = match config.listener_mode {
        crate::config::ListenerMode::UnixSocket => "unix-socket",
        crate::config::ListenerMode::TcpHttp => "tcp-http",
        crate::config::ListenerMode::TcpHttps => "tcp-https",
    }.to_string();

    // Get socket path (for Unix socket mode)
    let socket_path = if matches!(config.listener_mode, crate::config::ListenerMode::UnixSocket) {
        crate::config::socket_path()
            .ok()
            .map(|p| p.display().to_string())
    } else {
        None
    };

    // Get config file path
    let config_file_path = dirs::config_dir()
        .map(|dir| dir.join("ssh-tunnel-manager").join("daemon.toml").display().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Get current username
    let username = users::get_current_username()
        .and_then(|s| s.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());

    // Calculate SSH key directory (where daemon looks for keys)
    let ssh_key_dir = dirs::home_dir()
        .map(|home| home.join(".ssh").display().to_string())
        .unwrap_or_else(|| "~/.ssh".to_string());

    let info = DaemonInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        started_at: started_at_iso,
        listener_mode,
        bind_host: if matches!(config.listener_mode, crate::config::ListenerMode::TcpHttp | crate::config::ListenerMode::TcpHttps) {
            Some(config.bind_host.clone())
        } else {
            None
        },
        bind_port: if matches!(config.listener_mode, crate::config::ListenerMode::TcpHttp | crate::config::ListenerMode::TcpHttps) {
            Some(config.bind_port)
        } else {
            None
        },
        socket_path,
        require_auth: config.require_auth,
        group_access: config.group_access,
        config_file_path,
        known_hosts_path: config.known_hosts_path.display().to_string(),
        ssh_key_dir,
        active_tunnels_count: active_count,
        pid: std::process::id(),
        user: username,
    };

    Json(info)
}

/// Shutdown the daemon
async fn shutdown_daemon(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    info!("API: Shutdown request received");

    // Spawn a task to exit after a short delay
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        info!("Shutting down daemon...");
        std::process::exit(0);
    });

    StatusCode::ACCEPTED
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn heartbeat_stream_emits() {
        // With test interval override, we should see a heartbeat well within 1s.
        let mut stream = heartbeat_stream();
        let _evt = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .expect("heartbeat timed out")
            .expect("stream ended");

        // Ensure we emitted a heartbeat payload
        let json = heartbeat_payload();
        assert!(json.contains("heartbeat"), "heartbeat payload missing marker");
    }
}
