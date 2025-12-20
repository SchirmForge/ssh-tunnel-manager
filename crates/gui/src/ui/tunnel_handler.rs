// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// GTK-based tunnel event handler for SSE flow
//
// This module implements the TunnelEventHandler trait for GTK, enabling
// the GUI to use the shared SSE-first tunnel control flow from common.

use anyhow::Result;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::MessageDialogExt;
use std::sync::{Arc, Mutex};

use ssh_tunnel_common::{AuthRequest, DaemonTunnelEvent, Profile, TunnelEventHandler};

/// GTK event handler for tunnel operations
pub struct GtkTunnelEventHandler {
    profile: Profile,
    /// Channel for sending authentication responses from dialog back to handler
    auth_response_tx: Arc<Mutex<Option<async_channel::Sender<String>>>>,
    /// Window for showing dialogs (stored as glib::Object pointer, which is Send)
    window_ptr: usize,
}

// SAFETY: We only access the window from the main GTK thread via glib::MainContext::invoke
unsafe impl Send for GtkTunnelEventHandler {}

impl GtkTunnelEventHandler {
    /// Create a new GTK tunnel event handler
    pub fn new(
        profile: Profile,
        window: &adw::ApplicationWindow,
    ) -> Self {
        // Store the raw pointer as usize (which is Send)
        let window_ptr = window as *const _ as usize;

        Self {
            profile,
            auth_response_tx: Arc::new(Mutex::new(None)),
            window_ptr,
        }
    }

    /// Show authentication dialog and wait for response
    async fn show_auth_dialog_async(&self, prompt: &str, hidden: bool) -> Result<String> {
        // Create an async channel for the response
        let (tx, rx) = async_channel::bounded::<String>(1);

        // Store the sender so the dialog can use it
        {
            let mut guard = self.auth_response_tx.lock().unwrap();
            *guard = Some(tx);
        }

        // Show the dialog immediately (we're already on the main thread)
        let prompt_str = prompt.to_string();
        let auth_tx = self.auth_response_tx.clone();
        let window_ptr = self.window_ptr;

        // SAFETY: We know the window is still alive because the tunnel operation
        // is happening within the UI context. We're already on the main thread.
        unsafe {
            let window = &*(window_ptr as *const adw::ApplicationWindow);
            show_auth_dialog_internal(window, &prompt_str, hidden, auth_tx);
        }

        // Wait asynchronously for the response
        let response = rx
            .recv()
            .await
            .map_err(|_| anyhow::anyhow!("Authentication dialog was closed"))?;

        Ok(response)
    }
}

impl TunnelEventHandler for GtkTunnelEventHandler {
    fn on_auth_required(&mut self, request: &AuthRequest) -> Result<String> {
        // Block on the async dialog - this works because we're in a glib spawn_local context
        futures::executor::block_on(self.show_auth_dialog_async(&request.prompt, request.hidden))
    }

    fn on_connected(&mut self) {
        // Connection successful - could log or update UI here in the future
        eprintln!("✓ Tunnel connected! Forwarding {}:{} → {}:{}",
            self.profile.forwarding.bind_address,
            self.profile.forwarding.local_port.unwrap_or(0),
            self.profile
                .forwarding
                .remote_host
                .as_deref()
                .unwrap_or("?"),
            self.profile.forwarding.remote_port.unwrap_or(0)
        );
    }

    fn on_event(&mut self, event: &DaemonTunnelEvent) {
        // Log events - could be enhanced to update UI status labels in the future
        match event {
            DaemonTunnelEvent::Starting { .. } => {
                eprintln!("Connecting to SSH server...");
            }
            DaemonTunnelEvent::Connected { .. } => {
                // on_connected() will be called separately
            }
            DaemonTunnelEvent::Error { error, .. } => {
                eprintln!("Error: {}", error);
            }
            DaemonTunnelEvent::Disconnected { reason, .. } => {
                eprintln!("Disconnected: {}", reason);
            }
            DaemonTunnelEvent::AuthRequired { .. } => {
                eprintln!("Authentication required...");
            }
            DaemonTunnelEvent::Heartbeat { .. } => {
                // Ignore heartbeats
            }
        }
    }
}

/// Internal function to show the auth dialog
fn show_auth_dialog_internal(
    window: &adw::ApplicationWindow,
    prompt: &str,
    hidden: bool,
    auth_tx: Arc<Mutex<Option<async_channel::Sender<String>>>>,
) {
    let dialog = adw::MessageDialog::builder()
        .transient_for(window)
        .modal(true)
        .heading("Authentication Required")
        .body(prompt)
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
    dialog.connect_response(None, move |dialog, response| {
        if response == "submit" {
            // Get the text from the appropriate widget type
            let text = if let Some(password_entry) = entry_clone.downcast_ref::<gtk4::PasswordEntry>() {
                password_entry.text().to_string()
            } else if let Some(text_entry) = entry_clone.downcast_ref::<gtk4::Entry>() {
                text_entry.text().to_string()
            } else {
                String::new()
            };

            if !text.is_empty() {
                // Send the response through the async channel
                let mut guard = auth_tx.lock().unwrap();
                if let Some(tx) = guard.take() {
                    let _ = tx.try_send(text);
                }
            }
        } else {
            // User cancelled - drop the sender which causes recv() to fail
            let mut guard = auth_tx.lock().unwrap();
            if let Some(tx) = guard.take() {
                drop(tx);
            }
        }

        dialog.close();
    });

    dialog.present();
}
