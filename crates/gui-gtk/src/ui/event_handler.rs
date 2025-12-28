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
    eprintln!("Event: Status changed for profile {}: {:?}", profile_id, status);

    // Update status in AppCore
    {
        let mut core = state.core.borrow_mut();
        core.tunnel_statuses.insert(profile_id, status.clone());
    }

    // Update profiles list UI
    if let Some(list_box) = state.profile_list.borrow().as_ref() {
        profiles_list::update_profile_status(list_box, profile_id, status.clone());
    }

    // Clear auth state if connected/disconnected
    match &status {
        TunnelStatus::Connected | TunnelStatus::Disconnected | TunnelStatus::Failed(_) => {
            auth_dialog::clear_auth_state(state, profile_id);
        }
        _ => {}
    }
}

/// Handle an auth required event
pub fn handle_auth_required(state: &Rc<AppState>, request: AuthRequest) {
    eprintln!("Event: Auth required for profile {}: {}", request.tunnel_id, request.prompt);

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
    eprintln!("Event: Daemon connection changed: {}", connected);

    // Update AppCore state
    {
        let mut core = state.core.borrow_mut();
        core.daemon_connected = connected;
    }
}

/// Handle an error event
pub fn handle_error(state: &Rc<AppState>, profile_id: Option<Uuid>, error: String) {
    eprintln!("Event: Error{}: {}",
        profile_id.map(|id| format!(" for profile {}", id)).unwrap_or_default(),
        error
    );

    // If error is for a specific profile, update its status
    if let Some(id) = profile_id {
        let status = TunnelStatus::Failed(error.clone());

        // Update status in AppCore
        {
            let mut core = state.core.borrow_mut();
            core.tunnel_statuses.insert(id, status.clone());
        }

        // Clear auth state for this profile
        auth_dialog::clear_auth_state(state, id);

        // Update profiles list UI
        if let Some(list_box) = state.profile_list.borrow().as_ref() {
            profiles_list::update_profile_status(list_box, id, status.clone());
        }
    }

    // Show error toast/notification
    if let Some(window) = state.window.borrow().as_ref() {
        let toast = adw::Toast::new(&error);
        toast.set_timeout(5);

        // Try to get toast overlay from window
        if let Some(overlay) = window.child().and_then(|c| c.downcast::<adw::ToastOverlay>().ok()) {
            overlay.add_toast(toast);
        }
    }
}

/// Process a TunnelEvent from the SSE stream
pub fn process_tunnel_event(state: &Rc<AppState>, event: TunnelEvent) {
    match event {
        TunnelEvent::Connected { id } => {
            handle_status_changed(state, id, TunnelStatus::Connected);
        }
        TunnelEvent::Starting { id } => {
            handle_status_changed(state, id, TunnelStatus::Connecting);
        }
        TunnelEvent::Disconnected { id, reason } => {
            eprintln!("Tunnel {} disconnected: {}", id, reason);
            handle_status_changed(state, id, TunnelStatus::Disconnected);
        }
        TunnelEvent::Error { id, error } => {
            handle_error(state, Some(id), error);
        }
        TunnelEvent::AuthRequired { id: _, request } => {
            handle_auth_required(state, request);
        }
        TunnelEvent::Heartbeat { .. } => {
            // Heartbeat events are handled by the event listener for connection monitoring
        }
    }
}
