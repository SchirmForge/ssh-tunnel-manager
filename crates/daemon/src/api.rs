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

use ssh_tunnel_common::{load_profile_by_id, AuthRequest, AuthResponse, TunnelStatus};
use chrono::{DateTime, Utc};
use std::time::Duration;

use crate::tunnel::{TunnelEvent, TunnelManager};

/// Shared application state
pub struct AppState {
    pub tunnel_manager: TunnelManager,
    pub shutdown_tx: tokio::sync::broadcast::Sender<()>,
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
) -> impl IntoResponse {
    info!("API: Start tunnel request for {}", id);

    // Load the profile
    let profile = match load_profile_by_id(&id) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to load profile {}: {}", id, e);
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Profile not found: {}", e),
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
