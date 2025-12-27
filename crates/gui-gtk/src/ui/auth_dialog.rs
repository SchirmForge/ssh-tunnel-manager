// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Authentication dialog for handling password/2FA prompts

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::MessageDialogExt;
use std::rc::Rc;
use uuid::Uuid;

use super::{details, profile_details, profiles_list, window::AppState};
use ssh_tunnel_common::{AuthRequest, TunnelStatus};

/// Handle an authentication request, queueing if a dialog is already open.
pub fn handle_auth_request(
    parent: &adw::ApplicationWindow,
    request: AuthRequest,
    state: Rc<AppState>,
) {
    let tunnel_id = request.tunnel_id;
    {
        let mut core = state.core.borrow_mut();

        // Check if we're already handling this request
        if let Some(current) = core.active_auth_requests.get(&tunnel_id) {
            if is_same_request(current, &request) {
                return;
            }
        }

        // If dialog already open for this tunnel, queue the request
        if core.is_auth_dialog_open(tunnel_id) {
            if core.pending_auth_requests
                .get(&tunnel_id)
                .map(|existing| is_same_request(existing, &request))
                .unwrap_or(false)
            {
                return;
            }
            core.pending_auth_requests.insert(tunnel_id, request);
            return;
        }

        // Mark dialog as open and track the request
        core.mark_auth_dialog_open(tunnel_id);
        core.active_auth_requests.insert(tunnel_id, request.clone());
    }

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
    let parent = parent.clone();
    let dialog = adw::MessageDialog::builder()
        .transient_for(&parent)
        .modal(true)
        .heading("Authentication Required")
        .body(&prompt)
        .build();

    // Create appropriate entry widget based on whether input should be hidden
    let entry: gtk4::Widget = if hidden {
        gtk4::PasswordEntry::builder()
            .show_peek_icon(true)
            .placeholder_text("Enter password or code")
            .build()
            .upcast()
    } else {
        gtk4::Entry::builder()
            .placeholder_text("Enter response")
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
    let state = state.clone();
    let parent = parent.clone();
    let response_handled = std::cell::RefCell::new(false);
    dialog.connect_response(None, move |dialog, response| {
        if *response_handled.borrow() {
            return;
        }
        *response_handled.borrow_mut() = true;

        let mut cancelled = response != "submit";
        let mut submitted_text = None;

        if !cancelled {
            let text = if let Some(password_entry) = entry_clone.downcast_ref::<gtk4::PasswordEntry>() {
                password_entry.text().to_string()
            } else if let Some(text_entry) = entry_clone.downcast_ref::<gtk4::Entry>() {
                text_entry.text().to_string()
            } else {
                String::new()
            };

            if text.is_empty() {
                cancelled = true;
            } else {
                submitted_text = Some(text);
            }
        }

        if let Some(text) = submitted_text {
            let state = state.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Err(e) = submit_auth_async(profile_id, text, &state).await {
                    eprintln!("Failed to submit authentication: {}", e);
                }
            });
        } else if cancelled {
            let state = state.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Err(e) = cancel_auth_async(profile_id, &state).await {
                    eprintln!("Failed to cancel authentication: {}", e);
                }
            });
        }

        dialog.close();
        finish_auth_dialog(&parent, &state, profile_id, cancelled);
    });

    dialog.present();
}

/// Submit authentication response to daemon
async fn submit_auth_async(
    profile_id: Uuid,
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

    // Submit the auth response
    daemon_client.submit_auth(profile_id, response).await?;

    eprintln!("✓ Authentication submitted for tunnel {}", profile_id);

    // If the SSE event is missed, poll for a pending follow-up auth prompt (e.g., 2FA).
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    if let Ok(Some(request)) = daemon_client.get_pending_auth(profile_id).await {
        if let Some(window) = state.window.borrow().as_ref() {
            handle_auth_request(window, request, state.clone());
        }
    }

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
        eprintln!("Warning: Failed to stop tunnel after auth cancel: {}", e);
    }

    update_status_after_cancel(state, profile_id);
    Ok(())
}

fn finish_auth_dialog(
    parent: &adw::ApplicationWindow,
    state: &Rc<AppState>,
    profile_id: Uuid,
    cancelled: bool,
) {
    let next_request = {
        let mut core = state.core.borrow_mut();
        let next = if cancelled {
            core.pending_auth_requests.remove(&profile_id);
            None
        } else {
            core.pending_auth_requests.remove(&profile_id)
        };

        core.mark_auth_dialog_closed(profile_id);
        next
    };

    if let Some(request) = next_request {
        handle_auth_request(parent, request, state.clone());
    }
}

pub fn clear_auth_state(state: &Rc<AppState>, profile_id: Uuid) {
    let mut core = state.core.borrow_mut();
    core.mark_auth_dialog_closed(profile_id);
    core.pending_auth_requests.remove(&profile_id);
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

fn is_same_request(a: &AuthRequest, b: &AuthRequest) -> bool {
    a.tunnel_id == b.tunnel_id
        && a.auth_type == b.auth_type
        && a.prompt == b.prompt
        && a.hidden == b.hidden
}
