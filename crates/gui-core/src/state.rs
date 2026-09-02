// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Framework-agnostic application state

use ssh_tunnel_common::{AuthRequest, Profile, TunnelStatus};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Core application state (framework-agnostic).
///
/// Framework-specific state such as widgets belongs in the presentation
/// adapter. New adapters should use [`crate::AppController`]; this type remains
/// available for compatibility with the existing GTK implementation.
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

    /// Add pending auth request.
    ///
    /// Idempotent: delivering the same request twice leaves exactly one pending prompt. The
    /// daemon re-sends outstanding prompts to a newly connected subscriber, so a repeat is
    /// normal rather than exceptional — a request id identifies the *question*, not the
    /// delivery.
    pub fn add_pending_auth(&mut self, request: AuthRequest) {
        let request_id = request.id;
        let tunnel_id = request.tunnel_id;

        // Store by request ID
        self.pending_auth_requests.insert(request_id, request);

        // Update tunnel → request mapping, cleaning up any *superseded* request.
        if let Some(old_req_id) = self.tunnel_active_request.insert(tunnel_id, request_id) {
            // Guard against the id we just inserted: without this, a repeat of the same
            // request removed the entry added a line earlier and the prompt vanished.
            if old_req_id != request_id {
                self.pending_auth_requests.remove(&old_req_id);
                self.active_auth_requests.remove(&old_req_id);
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ssh_tunnel_common::AuthRequestType;

    fn request(tunnel_id: Uuid) -> AuthRequest {
        AuthRequest {
            id: Uuid::new_v4(),
            tunnel_id,
            auth_type: AuthRequestType::Password,
            prompt: "Enter SSH password: ".into(),
            hidden: true,
        }
    }

    /// The daemon re-sends outstanding prompts to a newly connected subscriber, so the same
    /// request arrives more than once whenever the stream reconnects. Before this was
    /// idempotent the repeat *removed* the prompt: the tunnel→request map returned the id
    /// just inserted, and the cleanup deleted it.
    #[test]
    fn delivering_the_same_request_twice_leaves_one_pending_prompt() {
        let mut state = AppCore::default();
        let req = request(Uuid::new_v4());

        state.add_pending_auth(req.clone());
        state.add_pending_auth(req.clone());

        assert_eq!(
            state.pending_auth_requests.len(),
            1,
            "a repeated delivery must not duplicate the prompt"
        );
        assert!(
            state.pending_auth_requests.contains_key(&req.id),
            "a repeated delivery must not erase the prompt"
        );
        assert_eq!(
            state.tunnel_active_request.get(&req.tunnel_id),
            Some(&req.id)
        );
    }

    /// A genuinely new question for the same tunnel — a rejected password, say — still
    /// supersedes the old one.
    #[test]
    fn a_new_request_for_the_same_tunnel_supersedes_the_old_one() {
        let tunnel_id = Uuid::new_v4();
        let mut state = AppCore::default();
        let first = request(tunnel_id);
        let second = request(tunnel_id);

        state.add_pending_auth(first.clone());
        state.add_pending_auth(second.clone());

        assert_eq!(state.pending_auth_requests.len(), 1);
        assert!(!state.pending_auth_requests.contains_key(&first.id));
        assert!(state.pending_auth_requests.contains_key(&second.id));
        assert_eq!(
            state.tunnel_active_request.get(&tunnel_id),
            Some(&second.id)
        );
    }

    #[test]
    fn requests_for_different_tunnels_coexist() {
        let mut state = AppCore::default();
        let a = request(Uuid::new_v4());
        let b = request(Uuid::new_v4());

        state.add_pending_auth(a.clone());
        state.add_pending_auth(b.clone());

        assert_eq!(state.pending_auth_requests.len(), 2);
    }
}
