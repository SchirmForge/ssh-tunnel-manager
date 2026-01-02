// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! GTK event handling utilities
//!
//! Note: The TunnelEventHandler trait requires Send + Sync, but GTK widgets and Rc
//! are not thread-safe. Instead of implementing the trait directly, we provide
//! utility functions that can be called from the event loop.

use gtk4::prelude::*;
use libadwaita as adw;
use std::rc::Rc;
use ssh_tunnel_common::{TunnelStatus, AuthRequest};
use uuid::Uuid;

use super::window::AppState;
use super::{profiles_list, auth_dialog};
use crate::daemon::TunnelEvent;

/// Handle a status change event
pub fn handle_status_changed(state: &Rc<AppState>, profile_id: Uuid, status: TunnelStatus) {
    tracing::debug!("Event: Status changed for profile {}: {:?}", profile_id, status);

    // Update status in AppCore
    {
        let mut core = state.core.borrow_mut();
        core.tunnel_statuses.insert(profile_id, status.clone());
    }

    // Update profiles list UI
    if let Some(list_box) = state.profile_list.borrow().as_ref() {
        profiles_list::update_profile_status(list_box, profile_id, status.clone());
    }

    // Clear auth state if terminal state reached
    // NOTE: Dialogs close themselves on user interaction (Submit/Cancel)
    // We should NOT call dialog.close() here to avoid GTK panic
    match &status {
        TunnelStatus::Connected => {
            // Auth succeeded - clear dialog reference (already closed by user)
            tracing::info!("Authentication successful for tunnel {}", profile_id);
            state.active_auth_dialog.replace(None);
            auth_dialog::clear_auth_state(state, profile_id);

            // Clear processing flag and process next queued request
            *state.processing_auth_request.borrow_mut() = false;
            if let Some(window) = state.window.borrow().as_ref() {
                auth_dialog::process_auth_queue(window, state.clone());
            }
        }
        TunnelStatus::Connecting => {
            // Intermediate auth step succeeded (e.g., passphrase accepted, now needs 2FA)
            // Clear current dialog reference and process next auth request from queue
            tracing::debug!("Auth step succeeded for tunnel {} - processing queue", profile_id);
            state.active_auth_dialog.replace(None);
            auth_dialog::clear_auth_state(state, profile_id);

            // Clear processing flag and process next queued request
            *state.processing_auth_request.borrow_mut() = false;
            if let Some(window) = state.window.borrow().as_ref() {
                auth_dialog::process_auth_queue(window, state.clone());
            }
        }
        TunnelStatus::Disconnected | TunnelStatus::Failed(_) => {
            // Auth failed or tunnel stopped - clear dialog reference
            state.active_auth_dialog.replace(None);
            auth_dialog::clear_auth_state(state, profile_id);

            // Clear queue for this tunnel (no more auth will come)
            state.auth_request_queue.borrow_mut().retain(|req| req.tunnel_id != profile_id);

            // Clear processing flag and process next tunnel's auth
            *state.processing_auth_request.borrow_mut() = false;
            if let Some(window) = state.window.borrow().as_ref() {
                auth_dialog::process_auth_queue(window, state.clone());
            }
        }
        _ => {}
    }
}

/// Handle an auth required event
pub fn handle_auth_required(state: &Rc<AppState>, request: AuthRequest) {
    tracing::debug!("Event: Auth required for profile {}: {}", request.tunnel_id, request.prompt);

    // Update status to WaitingForAuth in AppCore
    {
        let mut core = state.core.borrow_mut();
        core.tunnel_statuses.insert(request.tunnel_id, TunnelStatus::WaitingForAuth);
    }

    // Update profiles list UI
    if let Some(list_box) = state.profile_list.borrow().as_ref() {
        profiles_list::update_profile_status(list_box, request.tunnel_id, TunnelStatus::WaitingForAuth);
    }

    // Show auth dialog
    if let Some(window) = state.window.borrow().as_ref() {
        auth_dialog::handle_auth_request(window, request, state.clone());
    }
}

/// Handle daemon connection state change
pub fn handle_daemon_connected(state: &Rc<AppState>, connected: bool) {
    tracing::info!("Event: Daemon connection changed: {}", connected);

    // Update AppCore state
    {
        let mut core = state.core.borrow_mut();
        core.daemon_connected = connected;
    }
}

/// Handle an error event
pub fn handle_error(state: &Rc<AppState>, profile_id: Option<Uuid>, error: String) {
    tracing::info!("handle_error called - profile_id: {:?}, error: {}", profile_id, error);

    // If error is for a specific profile, update its status
    if let Some(id) = profile_id {
        tracing::info!("Updating status for profile {} to Failed", id);
        let status = TunnelStatus::Failed(error.clone());

        // Update status in AppCore
        {
            let mut core = state.core.borrow_mut();
            core.tunnel_statuses.insert(id, status.clone());
            tracing::info!("Status updated in AppCore");
        }

        // Clear auth state for this profile
        tracing::info!("Clearing auth state");
        auth_dialog::clear_auth_state(state, id);

        // Close the dialog
        tracing::info!("Checking for active dialog to close...");
        if let Some(dialog) = state.active_auth_dialog.borrow_mut().take() {
            dialog.close();
            tracing::info!("Closed active auth dialog");
        } else {
            tracing::warn!("No active dialog found to close");
        }

        // Clear queue for this tunnel (no more auth will come)
        state.auth_request_queue.borrow_mut().retain(|req| req.tunnel_id != id);
        tracing::info!("Cleared auth queue for tunnel {}", id);

        // Clear processing flag and process next tunnel's auth
        *state.processing_auth_request.borrow_mut() = false;
        if let Some(window) = state.window.borrow().as_ref() {
            auth_dialog::process_auth_queue(window, state.clone());
            tracing::info!("Processing next auth request from queue");
        }

        // Update profiles list UI
        tracing::info!("Updating profiles list UI");
        if let Some(list_box) = state.profile_list.borrow().as_ref() {
            profiles_list::update_profile_status(list_box, id, status.clone());
            tracing::info!("Profiles list UI updated");
        } else {
            tracing::info!("No profile list available to update");
        }
    }

    // Show error toast/notification
    tracing::info!("Attempting to show error toast");
    if let Some(window) = state.window.borrow().as_ref() {
        let toast = adw::Toast::new(&error);
        toast.set_timeout(5);

        // Try to get toast overlay from window
        if let Some(overlay) = window.child().and_then(|c| c.downcast::<adw::ToastOverlay>().ok()) {
            overlay.add_toast(toast);
            tracing::debug!("Toast shown successfully");
        } else {
            tracing::debug!("Could not get toast overlay from window");
        }
    } else {
        tracing::debug!("No window available to show toast");
    }

    tracing::debug!("handle_error completed");
}

/// Process a TunnelEvent from the SSE stream
pub fn process_tunnel_event(state: &Rc<AppState>, event: TunnelEvent) {
    tracing::debug!("process_tunnel_event called with: {:?}", event);

    match event {
        TunnelEvent::Connected { id } => {
            tracing::debug!("Processing Connected event for {}", id);
            handle_status_changed(state, id, TunnelStatus::Connected);
        }
        TunnelEvent::Starting { id } => {
            tracing::debug!("Processing Starting event for {}", id);
            handle_status_changed(state, id, TunnelStatus::Connecting);
        }
        TunnelEvent::Disconnected { id, reason } => {
            tracing::debug!("Processing Disconnected event for {}: {}", id, reason);
            handle_status_changed(state, id, TunnelStatus::Disconnected);
        }
        TunnelEvent::Error { id, error } => {
            tracing::debug!("Processing Error event for {}: {}", id, error);
            handle_error(state, Some(id), error);
        }
        TunnelEvent::AuthRequired { id: _, request } => {
            tracing::debug!("Processing AuthRequired event for {}", request.tunnel_id);
            handle_auth_required(state, request);
        }
        TunnelEvent::Heartbeat { .. } => {
            // Heartbeat events are handled by the event listener for connection monitoring
            // Don't log these - too noisy
        }
    }

    tracing::debug!("process_tunnel_event completed");
}
