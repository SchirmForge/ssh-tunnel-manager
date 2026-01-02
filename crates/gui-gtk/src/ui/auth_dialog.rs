// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Authentication dialog for handling password/2FA prompts

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::MessageDialogExt;
use std::rc::Rc;
use uuid::Uuid;

use super::{details, profile_details, profiles_list, window::AppState};
use ssh_tunnel_common::{AuthRequest, AuthRequestType, TunnelStatus};

/// Handle an authentication request by queuing it for processing.
/// Events are queued and processed sequentially to prevent GTK event loop overwhelm.
pub fn handle_auth_request(
    parent: &adw::ApplicationWindow,
    request: AuthRequest,
    state: Rc<AppState>,
) {
    // Check if this request is already being processed or queued (avoid duplicates from polling)
    let is_active = state.active_auth_request_id.borrow()
        .map(|id| id == request.id)
        .unwrap_or(false);

    if is_active {
        tracing::debug!("Auth request {} is currently active - skipping duplicate", request.id);
        return;
    }

    let already_queued = state.auth_request_queue.borrow()
        .iter()
        .any(|req| req.id == request.id);

    if already_queued {
        tracing::debug!("Auth request {} already queued - skipping duplicate", request.id);
        return;
    }

    tracing::debug!("Queueing auth request {} for tunnel {}", request.id, request.tunnel_id);

    // Queue the request
    state.auth_request_queue.borrow_mut().push_back(request);

    // Try to process the queue
    process_auth_queue(parent, state);
}

/// Process the next auth request in the queue (if not already processing).
/// This is public so event_handler.rs can call it when status events arrive.
pub fn process_auth_queue(
    parent: &adw::ApplicationWindow,
    state: Rc<AppState>,
) {
    // Check if we're already showing a dialog
    if *state.processing_auth_request.borrow() {
        tracing::debug!("Already processing auth request - queue size: {}",
                  state.auth_request_queue.borrow().len());
        return;
    }

    // Get next request from queue
    let request = match state.auth_request_queue.borrow_mut().pop_front() {
        Some(req) => req,
        None => {
            tracing::debug!("Auth queue empty");
            return;
        }
    };

    tracing::info!("Processing auth request {} from queue (remaining: {})",
              request.id, state.auth_request_queue.borrow().len());

    let tunnel_id = request.tunnel_id;
    let request_id = request.id;

    // Mark as processing
    *state.processing_auth_request.borrow_mut() = true;

    // Update tracked request ID
    state.active_auth_request_id.replace(Some(request_id));

    // Update core state
    {
        let mut core = state.core.borrow_mut();
        core.mark_auth_dialog_open(tunnel_id);
        core.active_auth_requests.insert(request_id, request.clone());
    }

    // Show dialog
    show_auth_dialog(parent, request, state);
}

/// Show authentication dialog for a tunnel
fn show_auth_dialog(
    parent: &adw::ApplicationWindow,
    request: AuthRequest,
    state: Rc<AppState>,
) {
    let profile_id = request.tunnel_id;
    let prompt = request.prompt.clone();
    let hidden = request.hidden;
    let auth_type = request.auth_type;
    let parent = parent.clone();
    let dialog = adw::MessageDialog::builder()
        .transient_for(&parent)
        .modal(true)
        .heading("Authentication Required")
        .body(&prompt)
        .build();

    // Determine appropriate placeholder text based on auth type
    let placeholder = match auth_type {
        AuthRequestType::KeyPassphrase => "Enter SSH key passphrase",
        AuthRequestType::Password => "Enter remote user password",
        AuthRequestType::TwoFactorCode => "Enter 2FA code",
        AuthRequestType::KeyboardInteractive => {
            if hidden {
                "Enter password or code"
            } else {
                "Enter response"
            }
        }
        AuthRequestType::HostKeyVerification => "Type 'yes' to accept or 'no' to reject",
    };

    // Create appropriate entry widget based on whether input should be hidden
    let entry: gtk4::Widget = if hidden {
        gtk4::PasswordEntry::builder()
            .show_peek_icon(true)
            .placeholder_text(placeholder)
            .build()
            .upcast()
    } else {
        gtk4::Entry::builder()
            .placeholder_text(placeholder)
            .build()
            .upcast()
    };

    // Create a box to hold the entry
    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    content_box.set_margin_start(12);
    content_box.set_margin_end(12);
    content_box.set_margin_top(12);
    content_box.set_margin_bottom(12);
    content_box.append(&entry);

    // Add content to dialog
    dialog.set_extra_child(Some(&content_box));

    // Add responses
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("submit", "Submit");
    dialog.set_response_appearance("submit", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("submit"));
    dialog.set_close_response("cancel");

    // Store dialog reference in state so we can close it on retry
    state.active_auth_dialog.replace(Some(dialog.clone()));

    // Focus the entry when dialog is shown
    entry.grab_focus();

    // Wire up Enter key to submit
    let dialog_clone = dialog.clone();
    if let Some(password_entry) = entry.downcast_ref::<gtk4::PasswordEntry>() {
        password_entry.connect_activate(move |_| {
            dialog_clone.response("submit");
        });
    } else if let Some(text_entry) = entry.downcast_ref::<gtk4::Entry>() {
        text_entry.connect_activate(move |_| {
            dialog_clone.response("submit");
        });
    }

    // Handle response
    let entry_clone = entry.clone();
    let state_clone = state.clone();
    let parent_clone = parent.clone();
    let response_handled = std::cell::RefCell::new(false);
    dialog.connect_response(None, move |dialog, response| {
        if *response_handled.borrow() {
            return;
        }
        *response_handled.borrow_mut() = true;

        let cancelled = response != "submit";

        if !cancelled {
            // Get the text from entry
            let text = if let Some(password_entry) = entry_clone.downcast_ref::<gtk4::PasswordEntry>() {
                password_entry.text().to_string()
            } else if let Some(text_entry) = entry_clone.downcast_ref::<gtk4::Entry>() {
                text_entry.text().to_string()
            } else {
                String::new()
            };

            if text.is_empty() {
                // Empty input treated as cancel
                tracing::debug!("Empty authentication response - treating as cancel");
                handle_cancel(dialog, &state_clone, &parent_clone, profile_id);
            } else {
                // Get request_id from state
                let request_id = match *state_clone.active_auth_request_id.borrow() {
                    Some(id) => id,
                    None => {
                        // Request already cleared (daemon timeout/failure) - treat as cancel
                        tracing::warn!("Auth request already cleared for tunnel {} (daemon timeout or failure)", profile_id);

                        // Show toast notification to user
                        let toast = adw::Toast::new("Authentication request was cancelled by daemon (timeout or failure)");
                        toast.set_timeout(5);
                        if let Some(overlay) = parent_clone.first_child()
                            .and_then(|w| w.downcast::<adw::ToastOverlay>().ok())
                        {
                            overlay.add_toast(toast);
                        }

                        handle_cancel(dialog, &state_clone, &parent_clone, profile_id);
                        return;
                    }
                };

                tracing::info!("Submitting auth for tunnel {} (request {})", profile_id, request_id);

                // Close dialog immediately - SSE events will handle next steps
                dialog.close();
                state_clone.active_auth_dialog.replace(None);

                // Clear processing flag to allow next auth request from queue
                *state_clone.processing_auth_request.borrow_mut() = false;

                let state = state_clone.clone();

                // Submit auth in background
                glib::MainContext::default().spawn_local(async move {
                    if let Err(e) = submit_auth_async(profile_id, request_id, text, &state).await {
                        tracing::error!("Failed to submit authentication for tunnel {}: {}", profile_id, e);
                        // Network error - daemon won't receive our response
                        // User will need to retry connection
                    }
                    // On success: SSE events will trigger appropriate actions:
                    // - Connected → status updated on main screen
                    // - AuthRequired (retry) → new dialog appears via queue
                    // - Error → error shown on main screen
                });
            }
        } else {
            // User clicked cancel
            handle_cancel(dialog, &state_clone, &parent_clone, profile_id);
        }
    });

    dialog.present();
}

/// Handle cancel button - close dialog and stop tunnel
fn handle_cancel(
    dialog: &adw::MessageDialog,
    state: &Rc<AppState>,
    parent: &adw::ApplicationWindow,
    profile_id: Uuid,
) {
    tracing::info!("Authentication cancelled for tunnel {}", profile_id);

    // Close dialog immediately on cancel
    dialog.close();

    // Clear dialog reference from state
    state.active_auth_dialog.replace(None);

    // Mark dialog as closed in core
    {
        let mut core = state.core.borrow_mut();
        core.mark_auth_dialog_closed(profile_id);
        core.pending_auth_requests.remove(&profile_id);
    }

    // Clear processing flag and process next queued request
    *state.processing_auth_request.borrow_mut() = false;

    // Stop the tunnel
    let state_for_cancel = state.clone();
    glib::MainContext::default().spawn_local(async move {
        if let Err(e) = cancel_auth_async(profile_id, &state_for_cancel).await {
            tracing::warn!("Failed to cancel authentication for tunnel {}: {}", profile_id, e);
        }
    });

    // Process next request from queue (if any)
    process_auth_queue(parent, state.clone());
}

/// Submit authentication response to daemon
async fn submit_auth_async(
    profile_id: Uuid,
    request_id: Uuid,
    response: String,
    state: &Rc<AppState>,
) -> anyhow::Result<()> {
    // Get daemon client
    let daemon_client = state
        .daemon_client
        .borrow()
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Daemon client not available"))?
        .clone();

    // Submit the auth response with request_id
    daemon_client.submit_auth_with_id(profile_id, request_id, response).await?;

    tracing::debug!("Authentication submitted for tunnel {} (request {})", profile_id, request_id);

    // SSE will handle next steps (success, retry, or error)
    // No polling needed - daemon will emit Connected/AuthRequired/Error event

    Ok(())
}

/// Cancel authentication by stopping the tunnel to avoid daemon timeout
async fn cancel_auth_async(profile_id: Uuid, state: &Rc<AppState>) -> anyhow::Result<()> {
    let daemon_client = state
        .daemon_client
        .borrow()
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Daemon client not available"))?
        .clone();

    if let Err(e) = daemon_client.stop_tunnel(profile_id).await {
        tracing::warn!("Failed to stop tunnel {} after auth cancel: {}", profile_id, e);
    }

    update_status_after_cancel(state, profile_id);
    Ok(())
}


pub fn clear_auth_state(state: &Rc<AppState>, profile_id: Uuid) {
    let mut core = state.core.borrow_mut();
    core.mark_auth_dialog_closed(profile_id);

    // Clear dialog references
    state.active_auth_dialog.replace(None);
    state.active_auth_request_id.replace(None);
}

fn update_status_after_cancel(state: &Rc<AppState>, profile_id: Uuid) {
    if let Some(list_box) = state.profile_list.borrow().as_ref() {
        profiles_list::update_profile_status(list_box, profile_id, TunnelStatus::Disconnected);
    }

    if let Some(selected) = state.selected_profile.borrow().as_ref() {
        if let Some(profile) = selected.profile() {
            if profile.metadata.id == profile_id {
                profile_details::update_tunnel_status(state, TunnelStatus::Disconnected);

                if let Some(details_widget) = state.details_widget.borrow().as_ref() {
                    if let Some(window) = state.window.borrow().as_ref() {
                        details::update_with_profile(details_widget, selected, state.clone(), window);
                    }
                }
            }
        }
    }
}
