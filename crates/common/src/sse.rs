// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Server-Sent Events listener for real-time tunnel status updates
//!
//! Framework-agnostic SSE client that works with any async runtime (tokio).

use anyhow::Result;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, Duration, Instant};
use uuid::Uuid;

use crate::{add_auth_header, AuthRequest, DaemonClientConfig};

/// Event from daemon SSE stream
/// Matches the daemon's OutgoingEvent structure
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

/// Event listener for daemon SSE stream
pub struct EventListener {
    config: DaemonClientConfig,
}

impl EventListener {
    /// Create a new event listener
    pub fn new(config: DaemonClientConfig) -> Self {
        Self { config }
    }

    /// Start listening to daemon events.
    ///
    /// Returns immediately with a channel receiver that yields [`TunnelEvent`] items, and
    /// reconnects on its own with exponential backoff if the stream drops. Suitable for a
    /// long-lived UI that should tolerate the daemon being briefly unreachable.
    pub async fn listen(&self) -> Result<mpsc::Receiver<TunnelEvent>> {
        Ok(self.spawn(None))
    }

    /// Like [`Self::listen`], but does not return until the stream is actually established.
    ///
    /// A caller that is about to *cause* events — starting a tunnel, say — must be subscribed
    /// before it acts, or it races the daemon for its own first events. Reporting a failure to
    /// connect up front also gives a far better message than a later, unexplained silence.
    pub async fn listen_ready(&self) -> Result<mpsc::Receiver<TunnelEvent>> {
        let (ready_tx, ready_rx) = oneshot::channel();
        let rx = self.spawn(Some(ready_tx));
        match ready_rx.await {
            Ok(Ok(())) => Ok(rx),
            Ok(Err(e)) => Err(e),
            Err(_) => anyhow::bail!("Event listener stopped before it connected"),
        }
    }

    /// Spawn the reconnecting stream task.
    ///
    /// `ready` is signalled by the **first** attempt only, success or failure; later
    /// reconnections are silent, since by then the caller is already listening.
    fn spawn(&self, mut ready: Option<oneshot::Sender<Result<()>>>) -> mpsc::Receiver<TunnelEvent> {
        let (tx, rx) = mpsc::channel(100);

        let config = self.config.clone();
        tokio::spawn(async move {
            let mut backoff = Backoff::default();

            loop {
                let started = Instant::now();
                if let Err(e) = Self::stream_events(&config, tx.clone(), ready.take()).await {
                    tracing::warn!("Event stream error: {}", e);
                }

                // If receiver is dropped, stop trying
                if tx.is_closed() {
                    break;
                }

                sleep(backoff.after_connection(started.elapsed())).await;
            }
        });

        rx
    }

    async fn stream_events(
        config: &DaemonClientConfig,
        tx: mpsc::Sender<TunnelEvent>,
        ready: Option<oneshot::Sender<Result<()>>>,
    ) -> Result<()> {
        let base_url = config.daemon_base_url()?;
        let url = format!("{}/api/events", base_url);

        // A streaming client: the request client's total timeout would cut this stream
        // after 30 seconds regardless of traffic. See `create_streaming_client`.
        let client = crate::create_streaming_client(config)?;

        // Build request with auth
        let request = client.get(&url);
        let request = add_auth_header(request, config)?;

        // Send request and get response stream
        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                let e = anyhow::Error::new(e).context("Failed to connect to event stream");
                if let Some(ready) = ready {
                    let _ = ready.send(Err(anyhow::anyhow!("{e:#}")));
                }
                return Err(e);
            }
        };

        if !response.status().is_success() {
            let message = event_stream_status_error(response.status());
            if let Some(ready) = ready {
                let _ = ready.send(Err(anyhow::anyhow!("{message}")));
            }
            anyhow::bail!("{message}");
        }

        if let Some(ready) = ready {
            let _ = ready.send(Ok(()));
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

/// Explain a non-success response from `/api/events`.
///
/// A 401 here almost always means the daemon has authentication enabled and the client has no
/// token, which is recoverable but not at all obvious from the status code alone. This lived
/// in the CLI's own inline subscription; it belongs here, so every caller gets it.
fn event_stream_status_error(status: reqwest::StatusCode) -> String {
    if status == reqwest::StatusCode::UNAUTHORIZED {
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
            .to_string()
    } else {
        format!("Daemon returned non-success status for events: {status}")
    }
}

/// Reconnect delay policy for the event stream.
///
/// Grows on repeated quick failures, and **resets after a connection that actually worked**.
/// The reset is the point: without it the delay only ever grew, so a few early blips pushed
/// it to the ceiling and left it there for the life of the process. Every later drop then
/// cost a full 30-second window in which daemon events were broadcast to nobody — including
/// authentication prompts, which expired with nothing shown to the user.
struct Backoff {
    current: Duration,
}

impl Backoff {
    const MIN: Duration = Duration::from_secs(1);
    const MAX: Duration = Duration::from_secs(30);
    /// A stream that ran at least this long counts as a working connection.
    const HEALTHY_AFTER: Duration = Duration::from_secs(10);

    /// Record a connection that lasted `lasted`, and return how long to wait before retrying.
    fn after_connection(&mut self, lasted: Duration) -> Duration {
        if lasted >= Self::HEALTHY_AFTER {
            self.current = Self::MIN;
        }
        let wait = self.current;
        self.current = (self.current * 2).min(Self::MAX);
        wait
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self { current: Self::MIN }
    }
}

impl Default for EventListener {
    fn default() -> Self {
        Self::new(DaemonClientConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSTANT: Duration = Duration::from_millis(50);
    const HEALTHY: Duration = Duration::from_secs(20);

    #[test]
    fn repeated_quick_failures_back_off() {
        let mut backoff = Backoff::default();
        let waits: Vec<u64> = (0..5)
            .map(|_| backoff.after_connection(INSTANT).as_secs())
            .collect();
        assert_eq!(waits, vec![1, 2, 4, 8, 16]);
    }

    #[test]
    fn backoff_is_capped() {
        let mut backoff = Backoff::default();
        for _ in 0..10 {
            backoff.after_connection(INSTANT);
        }
        assert_eq!(backoff.after_connection(INSTANT), Backoff::MAX);
    }

    /// The regression this type exists for: a working connection must clear the penalty
    /// accumulated by earlier failures, or the client ends up permanently half-disconnected.
    #[test]
    fn a_working_connection_resets_the_backoff() {
        let mut backoff = Backoff::default();
        for _ in 0..8 {
            backoff.after_connection(INSTANT);
        }
        assert_eq!(
            backoff.after_connection(HEALTHY),
            Backoff::MIN,
            "a stream that stayed up must not inherit the penalty from earlier blips"
        );
    }

    #[test]
    fn a_connection_that_barely_started_does_not_count_as_healthy() {
        let mut backoff = Backoff::default();
        backoff.after_connection(INSTANT);
        assert_eq!(backoff.after_connection(INSTANT), Duration::from_secs(2));
    }
}
