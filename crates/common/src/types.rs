// Common types for SSH Tunnel Manager

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Authentication type for SSH connection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
    /// SSH key authentication
    Key,
    /// Password authentication
    Password,
    /// Password + 2FA
    PasswordWith2FA,
}

/// Type of port forwarding
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ForwardingType {
    /// Local port forwarding (bind local port, forward to remote)
    Local,
    /// Remote port forwarding (bind remote port, forward to local)
    Remote,
    /// Dynamic port forwarding (SOCKS proxy)
    Dynamic,
}

/// Status of a tunnel connection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TunnelStatus {
    NotConnected,   // no active tunnel task
    Connecting,     // TCP + SSH handshake + key-exchange
    WaitingForAuth, // we sent AuthRequired to client
    Connected,      // port forwarding running
    Disconnecting,  // user/daemon is tearing down
    Disconnected,   // cleanly disconnected
    Reconnecting,   // reconnecting (not used by CG, but we might implement it later)
    Failed(String), // connection attempt failed (reason)
}

/// Events emitted by the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TunnelEvent {
    /// Tunnel connected successfully
    Starting { id: Uuid, timestamp: DateTime<Utc> },

    /// Tunnel connected successfully
    Connected { id: Uuid, timestamp: DateTime<Utc> },

    /// Tunnel disconnected
    Disconnected {
        id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    /// Tunnel is reconnecting
    Reconnecting {
        id: Uuid,
        attempt: u32,
        timestamp: DateTime<Utc>,
    },

    /// Tunnel error occurred
    Error {
        id: Uuid,
        error: String,
        timestamp: DateTime<Utc>,
    },

    AuthRequired {
        id: Uuid,
        prompt: String,
        timestamp: DateTime<Utc>,
    },

    /// 2FA code required
    TwoFactorRequired {
        id: Uuid,
        prompt: String,
        timestamp: DateTime<Utc>,
    },
}

/// Type of authentication input required from user
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthRequestType {
    /// SSH key passphrase needed
    KeyPassphrase,
    /// SSH password needed
    Password,
    /// 2FA/TOTP code needed
    TwoFactorCode,
    /// Keyboard-interactive prompt (generic)
    KeyboardInteractive,
    /// SSH host key verification needed (first connection or key changed)
    HostKeyVerification,
}

/// Authentication request from daemon to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    /// Tunnel ID this auth request is for
    pub tunnel_id: Uuid,
    /// Type of authentication needed
    pub auth_type: AuthRequestType,
    /// Prompt to display to user
    pub prompt: String,
    /// Whether input should be hidden (like passwords)
    pub hidden: bool,
}

/// Authentication response from client to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    /// Tunnel ID this response is for
    pub tunnel_id: Uuid,
    /// The user's input (password, code, etc.)
    pub response: String,
}

/// Status returned when starting a tunnel
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StartTunnelResult {
    /// Tunnel started and connected successfully
    Connected,
    /// Tunnel is connecting, may need authentication
    Connecting,
    /// Authentication is required - check /auth/pending endpoint
    AuthRequired { auth_request: AuthRequest },
    /// Tunnel failed to start
    Failed { error: String },
}

impl TunnelStatus {
    /// Check if the status represents an active connection
    pub fn is_connected(&self) -> bool {
        matches!(self, TunnelStatus::Connected,)
    }

    /// Check if the status represents a transitional state
    pub fn is_in_progress(&self) -> bool {
        matches!(
            self,
            TunnelStatus::Connecting
                | TunnelStatus::Disconnecting
                | TunnelStatus::Reconnecting
                | TunnelStatus::WaitingForAuth
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelStatusResponse {
    pub id: Uuid,
    pub status: TunnelStatus,
    pub pending_auth: Option<AuthRequest>,
}

/* impl TunnelStatus {
   pub fn is_really_connected(&self) -> bool {
       matches!(self, TunnelStatus::Connected)
   }

   pub fn is_in_progress(&self) -> bool {
       matches!(self, TunnelStatus::Connecting | TunnelStatus::WaitingForAuth | TunnelStatus::Disconnecting)
   }
} */
