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
    add_auth_header, create_daemon_client, prepare_profile_for_remote, AuthRequest, AuthResponse,
    ConnectionMode, DaemonClientConfig, DaemonInfo, Profile, ProfileSourceMode,
    StartTunnelRequest, TunnelStatus,
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

    /// Update the skip SSH setup warning preference
    pub fn set_skip_ssh_warning(&mut self, skip: bool) {
        self.config.skip_ssh_setup_warning = skip;
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

    /// Check if a profile requires SSH key setup warning for remote daemon
    pub async fn needs_ssh_key_warning(&self, profile: &Profile) -> Option<String> {
        use ssh_tunnel_common::get_remote_key_setup_message;

        // Skip if user has opted out of this warning
        if self.config.skip_ssh_setup_warning {
            return None;
        }

        // Only show warning for remote daemons
        let is_remote_daemon = matches!(
            self.config.connection_mode,
            ConnectionMode::Http | ConnectionMode::Https
        );

        if !is_remote_daemon {
            return None;
        }

        // Only show warning if profile uses SSH key authentication
        if let Some(key_path) = &profile.connection.key_path {
            let daemon_host = Some(self.config.daemon_host.as_str());

            // Try to fetch daemon info to get the actual SSH key directory
            let daemon_ssh_dir = self.get_daemon_info().await
                .ok()
                .map(|info| info.ssh_key_dir);

            Some(get_remote_key_setup_message(
                key_path,
                daemon_host,
                daemon_ssh_dir.as_deref(),
            ))
        } else {
            None
        }
    }

    /// Start a tunnel by profile
    pub async fn start_tunnel(&self, profile: &Profile) -> Result<()> {
        let profile_id = profile.metadata.id;

        // Determine if daemon is remote (HTTP/HTTPS) vs local (Unix socket)
        let is_remote_daemon = matches!(
            self.config.connection_mode,
            ConnectionMode::Http | ConnectionMode::Https
        );

        // Prepare the start tunnel request
        let (mode, profile_opt) = if is_remote_daemon {
            // Remote daemon - send profile via API
            let remote_profile = prepare_profile_for_remote(profile)
                .context("Failed to prepare profile for remote daemon")?;

            (ProfileSourceMode::Hybrid, Some(remote_profile))
        } else {
            // Local daemon (Unix socket) - load from filesystem
            (ProfileSourceMode::Local, None)
        };

        let start_request = StartTunnelRequest {
            profile_id: profile_id.to_string(),
            mode,
            profile: profile_opt,
        };

        let url = format!("{}/api/tunnels/{}/start", self.base_url()?, profile_id);
        let request = self.client.post(&url);
        let request = add_auth_header(request, &self.config)?;

        let response = request
            .json(&start_request)
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

    /// Submit authentication response for a tunnel with request ID
    pub async fn submit_auth_with_id(
        &self,
        profile_id: Uuid,
        request_id: Uuid,
        auth_response: String,
    ) -> Result<()> {
        let url = format!("{}/api/tunnels/{}/auth", self.base_url()?, profile_id);
        let request = self.client.post(&url);
        let request = add_auth_header(request, &self.config)?;

        let payload = serde_json::json!({
            "request_id": request_id,
            "response": auth_response,
        });

        let response = request
            .json(&payload)
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
