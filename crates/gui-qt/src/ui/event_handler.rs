// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Qt event handling utilities
//!
//! Similar to GTK's event_handler, but uses Qt signals/slots.
//! The key insight: the business logic is identical, only the UI updates differ.

use std::rc::Rc;
use crate::AppState;
use ssh_tunnel_common::{TunnelStatus, AuthRequest};
use uuid::Uuid;

// TODO: Add Qt imports for SSE
// use crate::daemon::sse::TunnelEvent;

/// Handle a status change event
///
/// Code reuse:
/// - Update AppCore state - SHARED logic
/// - Update Qt UI - Qt-specific
pub fn handle_status_changed(state: &Rc<AppState>, profile_id: Uuid, status: TunnelStatus) {
    eprintln!("Event: Status changed for profile {}: {:?}", profile_id, status);

    // Update status in AppCore (SAME as GTK!)
    {
        let mut core = state.core.borrow_mut();
        core.tunnel_statuses.insert(profile_id, status.clone());
    }

    // Update profiles list UI (Qt-specific)
    super::profiles_list::update_profile_status(profile_id, status.clone());

    // Clear auth state if appropriate (SAME logic as GTK!)
    match &status {
        TunnelStatus::Connected | TunnelStatus::Disconnected | TunnelStatus::Failed(_) => {
            // Clear auth state from AppCore
            let mut core = state.core.borrow_mut();
            core.mark_auth_dialog_closed(profile_id);
            core.pending_auth_requests.remove(&profile_id);
        }
        _ => {}
    }
}

/// Handle an auth required event
///
/// Code reuse:
/// - Update AppCore state - SHARED
/// - Show Qt dialog - Qt-specific
pub fn handle_auth_required(state: &Rc<AppState>, request: AuthRequest) {
    eprintln!("Event: Auth required for profile {}: {}", request.tunnel_id, request.prompt);

    // Update status in AppCore (SAME as GTK!)
    {
        let mut core = state.core.borrow_mut();
        core.tunnel_statuses.insert(request.tunnel_id, TunnelStatus::WaitingForAuth);
    }

    // Update profiles list UI
    super::profiles_list::update_profile_status(request.tunnel_id, TunnelStatus::WaitingForAuth);

    // Show auth dialog (Qt-specific)
    // super::auth_dialog::show(state, request);
}

/// Handle daemon connection state change
///
/// Code reuse:
/// - Update AppCore - SHARED
/// - Update Qt status indicator - Qt-specific
pub fn handle_daemon_connected(state: &Rc<AppState>, connected: bool) {
    eprintln!("Event: Daemon connection changed: {}", connected);

    // Update AppCore state (SAME as GTK!)
    {
        let mut core = state.core.borrow_mut();
        core.daemon_connected = connected;
    }

    // Update Qt status icon/indicator (Qt-specific)
    // if connected {
    //     status_icon.set_icon(QIcon::from_theme("network-wired"));
    //     status_icon.set_tooltip("Connected to daemon");
    // } else {
    //     status_icon.set_icon(QIcon::from_theme("network-offline"));
    //     status_icon.set_tooltip("Disconnected from daemon");
    // }
}

/// Handle an error event
pub fn handle_error(state: &Rc<AppState>, profile_id: Option<Uuid>, error: String) {
    eprintln!("Event: Error{}: {}",
        profile_id.map(|id| format!(" for profile {}", id)).unwrap_or_default(),
        error
    );

    // If error is for a specific profile, update its status (SAME as GTK!)
    if let Some(id) = profile_id {
        let status = TunnelStatus::Failed(error.clone());

        {
            let mut core = state.core.borrow_mut();
            core.tunnel_statuses.insert(id, status.clone());
        }

        super::profiles_list::update_profile_status(id, status);
    }

    // Show error notification (Qt-specific)
    // QMessageBox::warning(..., error);
}

/// Process a TunnelEvent from the SSE stream
///
/// This function is ALMOST IDENTICAL to the GTK version!
/// Only the UI update functions are different.
pub fn process_tunnel_event(state: &Rc<AppState>, event: &str) {
    // Parse event JSON (would use TunnelEvent enum)
    // match event {
    //     TunnelEvent::Connected { id } => {
    //         handle_status_changed(state, id, TunnelStatus::Connected);
    //     }
    //     TunnelEvent::Starting { id } => {
    //         handle_status_changed(state, id, TunnelStatus::Connecting);
    //     }
    //     TunnelEvent::Disconnected { id, reason } => {
    //         eprintln!("Tunnel {} disconnected: {}", id, reason);
    //         handle_status_changed(state, id, TunnelStatus::Disconnected);
    //     }
    //     TunnelEvent::Error { id, error } => {
    //         handle_error(state, Some(id), error);
    //     }
    //     TunnelEvent::AuthRequired { id, request } => {
    //         handle_auth_required(state, request);
    //     }
    //     TunnelEvent::Heartbeat { .. } => {
    //         handle_daemon_connected(state, true);
    //     }
    // }

    eprintln!("Would process event: {}", event);
}
