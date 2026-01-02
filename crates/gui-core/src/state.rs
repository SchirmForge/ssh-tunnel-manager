// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Framework-agnostic application state

use ssh_tunnel_common::{Profile, TunnelStatus, AuthRequest};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Core application state (framework-agnostic)
///
/// This contains all the business logic state that is shared between
/// GTK and Qt implementations. Framework-specific state (widgets, etc.)
/// should be stored in the framework-specific implementation.
#[derive(Debug)]
pub struct AppCore {
    /// All loaded profiles
    pub profiles: Vec<Profile>,

    /// Current tunnel status for each profile
    pub tunnel_statuses: HashMap<Uuid, TunnelStatus>,

    /// Whether daemon is connected
    pub daemon_connected: bool,

    /// Pending authentication requests (keyed by request.id)
    pub pending_auth_requests: HashMap<Uuid, AuthRequest>,

    /// Active authentication requests (dialog currently shown, keyed by request.id)
    pub active_auth_requests: HashMap<Uuid, AuthRequest>,

    /// Mapping from tunnel_id to current active request_id
    pub tunnel_active_request: HashMap<Uuid, Uuid>,

    /// Track which profiles have auth dialogs open (still keyed by tunnel_id)
    pub auth_dialog_open: HashSet<Uuid>,

    /// Current navigation page
    pub current_page: Page,
}

/// Application pages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// Profile list page
    Client,
    /// Profile details page
    ProfileDetails(Uuid),
    /// Daemon status page
    Daemon,
    /// Client configuration page
    ClientConfig,
}

impl Default for AppCore {
    fn default() -> Self {
        Self::new()
    }
}

impl AppCore {
    /// Create new application state
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
            tunnel_statuses: HashMap::new(),
            daemon_connected: false,
            pending_auth_requests: HashMap::new(),
            active_auth_requests: HashMap::new(),
            tunnel_active_request: HashMap::new(),
            auth_dialog_open: HashSet::new(),
            current_page: Page::Client,
        }
    }

    /// Load profiles from disk
    pub fn load_profiles(&mut self) -> anyhow::Result<()> {
        self.profiles = crate::profiles::load_profiles()?;
        Ok(())
    }

    /// Get profile by ID
    pub fn get_profile(&self, id: Uuid) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.metadata.id == id)
    }

    /// Get profile status
    pub fn get_status(&self, id: Uuid) -> TunnelStatus {
        self.tunnel_statuses
            .get(&id)
            .cloned()
            .unwrap_or(TunnelStatus::NotConnected)
    }

    /// Update profile status
    pub fn set_status(&mut self, id: Uuid, status: TunnelStatus) {
        self.tunnel_statuses.insert(id, status);
    }

    /// Set daemon connection state
    pub fn set_daemon_connected(&mut self, connected: bool) {
        self.daemon_connected = connected;

        // Clear all statuses when daemon disconnects
        if !connected {
            for status in self.tunnel_statuses.values_mut() {
                *status = TunnelStatus::NotConnected;
            }
        }
    }

    /// Add pending auth request
    pub fn add_pending_auth(&mut self, request: AuthRequest) {
        let request_id = request.id;
        let tunnel_id = request.tunnel_id;

        // Store by request ID
        self.pending_auth_requests.insert(request_id, request);

        // Update tunnel → request mapping, cleaning up old request if any
        if let Some(old_req_id) = self.tunnel_active_request.insert(tunnel_id, request_id) {
            // Clean up old request
            self.pending_auth_requests.remove(&old_req_id);
            self.active_auth_requests.remove(&old_req_id);
        }
    }

    /// Remove pending auth request by tunnel ID
    pub fn remove_pending_auth(&mut self, tunnel_id: Uuid) -> Option<AuthRequest> {
        if let Some(request_id) = self.tunnel_active_request.remove(&tunnel_id) {
            self.pending_auth_requests.remove(&request_id)
        } else {
            None
        }
    }

    /// Mark auth dialog as open for a tunnel
    pub fn mark_auth_dialog_open(&mut self, tunnel_id: Uuid) {
        self.auth_dialog_open.insert(tunnel_id);

        // Move from pending to active
        if let Some(request_id) = self.tunnel_active_request.get(&tunnel_id) {
            if let Some(request) = self.pending_auth_requests.remove(request_id) {
                self.active_auth_requests.insert(*request_id, request);
            }
        }
    }

    /// Mark auth dialog as closed for a tunnel
    pub fn mark_auth_dialog_closed(&mut self, tunnel_id: Uuid) {
        self.auth_dialog_open.remove(&tunnel_id);

        // Clear request_id from tunnel mapping and remove from active requests
        if let Some(request_id) = self.tunnel_active_request.remove(&tunnel_id) {
            self.active_auth_requests.remove(&request_id);
            self.pending_auth_requests.remove(&request_id);
        }
    }

    /// Get active request for a tunnel
    pub fn get_active_request_for_tunnel(&self, tunnel_id: Uuid) -> Option<&AuthRequest> {
        self.tunnel_active_request
            .get(&tunnel_id)
            .and_then(|req_id| self.active_auth_requests.get(req_id))
    }

    /// Check if auth dialog is open for a tunnel
    pub fn is_auth_dialog_open(&self, tunnel_id: Uuid) -> bool {
        self.auth_dialog_open.contains(&tunnel_id)
    }

    /// Navigate to a page
    pub fn navigate_to(&mut self, page: Page) {
        self.current_page = page;
    }
}
