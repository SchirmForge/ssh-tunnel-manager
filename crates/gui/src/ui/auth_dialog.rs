// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Authentication dialog for handling password/2FA prompts

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::MessageDialogExt;
use std::rc::Rc;
use uuid::Uuid;

use super::window::AppState;

/// Show authentication dialog for a tunnel
pub fn show_auth_dialog(
    parent: &adw::ApplicationWindow,
    profile_id: Uuid,
    prompt: &str,
    state: Rc<AppState>,
) {
    let dialog = adw::MessageDialog::builder()
        .transient_for(parent)
        .modal(true)
        .heading("Authentication Required")
        .body(prompt)
        .build();

    // Create password entry
    let password_entry = gtk4::PasswordEntry::builder()
        .show_peek_icon(true)
        .placeholder_text("Enter password or code")
        .build();

    // Create a box to hold the entry
    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    content_box.set_margin_start(12);
    content_box.set_margin_end(12);
    content_box.set_margin_top(12);
    content_box.set_margin_bottom(12);
    content_box.append(&password_entry);

    // Add content to dialog
    dialog.set_extra_child(Some(&content_box));

    // Add responses
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("submit", "Submit");
    dialog.set_response_appearance("submit", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("submit"));
    dialog.set_close_response("cancel");

    // Focus the password entry when dialog is shown
    password_entry.grab_focus();

    // Wire up Enter key to submit
    let dialog_clone = dialog.clone();
    password_entry.connect_activate(move |_| {
        dialog_clone.response("submit");
    });

    // Handle response
    let password_entry_clone = password_entry.clone();
    dialog.connect_response(None, move |dialog, response| {
        if response == "submit" {
            let password = password_entry_clone.text().to_string();

            if !password.is_empty() {
                // Spawn async task to submit auth
                let state = state.clone();
                glib::MainContext::default().spawn_local(async move {
                    if let Err(e) = submit_auth_async(profile_id, password, &state).await {
                        eprintln!("Failed to submit authentication: {}", e);
                    }
                });
            }
        }

        dialog.close();
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

    Ok(())
}
