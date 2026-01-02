// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Server-Sent Events listener for real-time tunnel status updates
//!
//! Framework-agnostic SSE client that works with any async runtime (tokio).

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::{add_auth_header, AuthRequest, DaemonClientConfig};

/// Event from daemon SSE stream
/// Matches the daemon's OutgoingEvent structure
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TunnelEvent {
    Starting {
        id: Uuid,
    },
    Connected {
        id: Uuid,
    },
    Disconnected {
        id: Uuid,
        reason: String,
    },
    Error {
        id: Uuid,
        error: String,
    },
    AuthRequired {
        id: Uuid,
        request: AuthRequest,
    },
    Heartbeat {
        timestamp: DateTime<Utc>,
    },
}

/// Event listener for daemon SSE stream
pub struct EventListener {
    config: DaemonClientConfig,
}

impl EventListener {
    /// Create a new event listener
    pub fn new(config: DaemonClientConfig) -> Self {
        Self { config }
    }

    /// Start listening to daemon events
    /// Returns a channel receiver that yields TunnelEvent items.
    /// Automatically reconnects with exponential backoff if the stream drops.
    pub async fn listen(&self) -> Result<mpsc::Receiver<TunnelEvent>> {
        let (tx, rx) = mpsc::channel(100);

        let config = self.config.clone();
        tokio::spawn(async move {
            let mut backoff = Duration::from_secs(1);
            let max_backoff = Duration::from_secs(30);

            loop {
                if let Err(e) = Self::stream_events(&config, tx.clone()).await {
                    tracing::warn!("Event stream error: {}", e);
                }

                // If receiver is dropped, stop trying
                if tx.is_closed() {
                    break;
                }

                sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        });

        Ok(rx)
    }

    async fn stream_events(
        config: &DaemonClientConfig,
        tx: mpsc::Sender<TunnelEvent>,
    ) -> Result<()> {
        let base_url = config.daemon_base_url()?;
        let url = format!("{}/api/events", base_url);

        // Create HTTP client
        let client = crate::create_daemon_client(config)?;

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

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    // Convert bytes to string
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        buffer.push_str(text);

                        // Process complete SSE messages
                        while let Some(pos) = buffer.find("\n\n") {
                            let message = buffer[..pos].to_string();
                            buffer = buffer[pos + 2..].to_string();

                            tracing::debug!("Raw SSE message: {:?}", message);

                            // Parse SSE message
                            if let Some(event) = Self::parse_sse_message(&message) {
                                tracing::debug!("Sending event to channel: {:?}", event);
                                if tx.send(event).await.is_err() {
                                    tracing::debug!("Receiver dropped, stopping event listener");
                                    // Receiver dropped, stop listening
                                    return Ok(());
                                }
                                tracing::debug!("Event sent to channel successfully");
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Error reading event stream: {}", e);
                    break;
                }
            }

            // If stream ends naturally, break to allow reconnect/backoff
        }

        Ok(())
    }

    /// Parse an SSE message into a TunnelEvent
    fn parse_sse_message(message: &str) -> Option<TunnelEvent> {
        // SSE format: "data: {json}\n"
        for line in message.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                // Parse JSON
                match serde_json::from_str::<TunnelEvent>(data) {
                    Ok(event) => {
                        tracing::debug!("Parsed SSE event: {:?}", event);
                        return Some(event);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse SSE event: {} (error: {})", data, e);
                    }
                }
            }
        }
        None
    }
}

impl Default for EventListener {
    fn default() -> Self {
        Self::new(DaemonClientConfig::default())
    }
}
