// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Daemon client for tunnel control operations
//!
//! Supports multiple connection modes:
//! - Unix socket (default, local-only)
//! - HTTP (testing/localhost, no TLS)
//! - HTTPS (network-ready with TLS and optional certificate pinning)

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

use ssh_tunnel_common::{
    add_auth_header, create_daemon_client, AuthRequest, AuthResponse, DaemonClientConfig,
    DaemonInfo, TunnelStatus,
};

/// Daemon client for tunnel operations
#[derive(Clone)]
pub struct DaemonClient {
    client: Client,
    pub config: DaemonClientConfig,
}

/// Response from start/stop operations
#[derive(Debug, Deserialize)]
pub struct OperationResponse {
    #[allow(dead_code)]
    pub message: String,
}

/// Error response from daemon API
#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Tunnel status response
#[derive(Debug, Deserialize)]
pub struct TunnelStatusResponse {
    pub id: Uuid,
    pub status: TunnelStatus,
    pub pending_auth: Option<AuthRequest>,
}

/// List of active tunnels
#[derive(Debug, Deserialize)]
pub struct TunnelsListResponse {
    pub tunnels: Vec<TunnelStatusResponse>,
}

impl DaemonClient {
    /// Create a new daemon client with default configuration
    pub fn new() -> Result<Self> {
        let config = DaemonClientConfig::default();
        let client = create_daemon_client(&config)?;

        Ok(Self { client, config })
    }

    /// Create a daemon client with custom configuration
    pub fn with_config(config: DaemonClientConfig) -> Result<Self> {
        let client = create_daemon_client(&config)?;
        Ok(Self { client, config })
    }

    /// Get the base URL for API requests
    fn base_url(&self) -> Result<String> {
        self.config.daemon_base_url()
    }

    /// Check daemon health
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/health", self.base_url()?);
        let request = self.client.get(&url);
        let request = add_auth_header(request, &self.config)?;

        match request.send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Start a tunnel by profile ID
    pub async fn start_tunnel(&self, profile_id: Uuid) -> Result<()> {
        let url = format!("{}/api/tunnels/{}/start", self.base_url()?, profile_id);
        let request = self.client.post(&url);
        let request = add_auth_header(request, &self.config)?;

        let response = request
            .send()
            .await
            .context("Failed to send start request")?;

        if response.status().is_success() {
            Ok(())
        } else {
            let error: ErrorResponse = response
                .json()
                .await
                .unwrap_or_else(|_| ErrorResponse {
                    error: "Unknown error".to_string(),
                });
            anyhow::bail!("Failed to start tunnel: {}", error.error)
        }
    }

    /// Stop a tunnel by profile ID
    pub async fn stop_tunnel(&self, profile_id: Uuid) -> Result<()> {
        let url = format!("{}/api/tunnels/{}/stop", self.base_url()?, profile_id);
        let request = self.client.post(&url);
        let request = add_auth_header(request, &self.config)?;

        let response = request
            .send()
            .await
            .context("Failed to send stop request")?;

        if response.status().is_success() {
            Ok(())
        } else {
            let error: ErrorResponse = response
                .json()
                .await
                .unwrap_or_else(|_| ErrorResponse {
                    error: "Unknown error".to_string(),
                });
            anyhow::bail!("Failed to stop tunnel: {}", error.error)
        }
    }

    /// Get tunnel status by profile ID
    pub async fn get_tunnel_status(&self, profile_id: Uuid) -> Result<Option<TunnelStatusResponse>> {
        let url = format!("{}/api/tunnels/{}/status", self.base_url()?, profile_id);
        let request = self.client.get(&url);
        let request = add_auth_header(request, &self.config)?;

        let response = request
            .send()
            .await
            .context("Failed to send status request")?;

        if response.status().is_success() {
            let status: TunnelStatusResponse = response
                .json()
                .await
                .context("Failed to parse status response")?;
            Ok(Some(status))
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            Ok(None)
        } else {
            let error: ErrorResponse = response
                .json()
                .await
                .unwrap_or_else(|_| ErrorResponse {
                    error: "Unknown error".to_string(),
                });
            anyhow::bail!("Failed to get tunnel status: {}", error.error)
        }
    }

    /// List all active tunnels
    pub async fn list_tunnels(&self) -> Result<Vec<TunnelStatusResponse>> {
        let url = format!("{}/api/tunnels", self.base_url()?);
        let request = self.client.get(&url);
        let request = add_auth_header(request, &self.config)?;

        let response = request
            .send()
            .await
            .context("Failed to send list request")?;

        if response.status().is_success() {
            let list: TunnelsListResponse = response
                .json()
                .await
                .context("Failed to parse tunnels list")?;
            Ok(list.tunnels)
        } else {
            let error: ErrorResponse = response
                .json()
                .await
                .unwrap_or_else(|_| ErrorResponse {
                    error: "Unknown error".to_string(),
                });
            anyhow::bail!("Failed to list tunnels: {}", error.error)
        }
    }

    /// Get pending authentication request for a tunnel
    pub async fn get_pending_auth(&self, profile_id: Uuid) -> Result<Option<AuthRequest>> {
        let url = format!("{}/api/tunnels/{}/auth", self.base_url()?, profile_id);
        let request = self.client.get(&url);
        let request = add_auth_header(request, &self.config)?;

        let response = request
            .send()
            .await
            .context("Failed to send auth request")?;

        if response.status().is_success() {
            let auth_request: AuthRequest = response
                .json()
                .await
                .context("Failed to parse auth request")?;
            Ok(Some(auth_request))
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            Ok(None)
        } else {
            let error: ErrorResponse = response
                .json()
                .await
                .unwrap_or_else(|_| ErrorResponse {
                    error: "Unknown error".to_string(),
                });
            anyhow::bail!("Failed to get pending auth: {}", error.error)
        }
    }

    /// Submit authentication response for a tunnel
    pub async fn submit_auth(&self, profile_id: Uuid, auth_response: String) -> Result<()> {
        let url = format!("{}/api/tunnels/{}/auth", self.base_url()?, profile_id);
        let request = self.client.post(&url);
        let request = add_auth_header(request, &self.config)?;

        let auth = AuthResponse {
            tunnel_id: profile_id,
            response: auth_response,
        };

        let response = request
            .json(&auth)
            .send()
            .await
            .context("Failed to send auth response")?;

        if response.status().is_success() {
            Ok(())
        } else {
            let error: ErrorResponse = response
                .json()
                .await
                .unwrap_or_else(|_| ErrorResponse {
                    error: "Unknown error".to_string(),
                });
            anyhow::bail!("Failed to submit auth: {}", error.error)
        }
    }

    /// Get daemon information (version, config, uptime, etc.)
    pub async fn get_daemon_info(&self) -> Result<DaemonInfo> {
        let url = format!("{}/api/daemon/info", self.base_url()?);
        let request = self.client.get(&url);
        let request = add_auth_header(request, &self.config)?;

        let response = request
            .send()
            .await
            .context("Failed to send daemon info request")?;

        if response.status().is_success() {
            let info: DaemonInfo = response
                .json()
                .await
                .context("Failed to parse daemon info")?;
            Ok(info)
        } else {
            let error: ErrorResponse = response
                .json()
                .await
                .unwrap_or_else(|_| ErrorResponse {
                    error: "Unknown error".to_string(),
                });
            anyhow::bail!("Failed to get daemon info: {}", error.error)
        }
    }

    /// Shutdown the daemon
    pub async fn shutdown_daemon(&self) -> Result<()> {
        let url = format!("{}/api/daemon/shutdown", self.base_url()?);
        let request = self.client.post(&url);
        let request = add_auth_header(request, &self.config)?;

        let response = request
            .send()
            .await
            .context("Failed to send shutdown request")?;

        if response.status().is_success() || response.status() == reqwest::StatusCode::ACCEPTED {
            Ok(())
        } else {
            let error: ErrorResponse = response
                .json()
                .await
                .unwrap_or_else(|_| ErrorResponse {
                    error: "Unknown error".to_string(),
                });
            anyhow::bail!("Failed to shutdown daemon: {}", error.error)
        }
    }
}

impl Default for DaemonClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default daemon client")
    }
}
