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
    auth_response_tx: Arc<Mutex<Option<std::sync::mpsc::Sender<Option<String>>>>>,
    /// Window for showing dialogs (stored as glib::Object pointer, which is Send)
    window_ptr: usize,
    /// Flag to track if auth has been cancelled (prevents showing subsequent dialogs)
    cancelled: Arc<Mutex<bool>>,
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
            cancelled: Arc::new(Mutex::new(false)),
        }
    }

    /// Show authentication dialog and wait for response synchronously
    fn show_auth_dialog_sync(&self, prompt: &str, hidden: bool) -> Result<String> {
        // Create a synchronous channel for the response
        let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();

        // Store the sender so the dialog can use it
        {
            let mut guard = self.auth_response_tx.lock().unwrap();
            *guard = Some(tx);
        }

        // Show the dialog on the main thread using glib::idle_add
        let prompt_str = prompt.to_string();
        let auth_tx = self.auth_response_tx.clone();
        let window_ptr = self.window_ptr;

        glib::idle_add_once(move || {
            // SAFETY: We know the window is still alive because the tunnel operation
            // is happening within the UI context.
            unsafe {
                let window = &*(window_ptr as *const adw::ApplicationWindow);
                show_auth_dialog_internal(window, &prompt_str, hidden, auth_tx);
            }
        });

        // Block waiting for response - this is safe because we're in a separate
        // task context spawned by glib::spawn_local, not the main event loop
        let response = rx
            .recv()
            .map_err(|_| anyhow::anyhow!("Authentication dialog was closed"))?
            .ok_or_else(|| anyhow::anyhow!("Authentication was cancelled"))?;

        Ok(response)
    }
}

impl TunnelEventHandler for GtkTunnelEventHandler {
    fn on_auth_required(&mut self, request: &AuthRequest) -> Result<String> {
        eprintln!("[AUTH] on_auth_required called for tunnel {}", request.tunnel_id);
        eprintln!("[AUTH] Prompt: {}", request.prompt);
        eprintln!("[AUTH] Hidden: {}", request.hidden);

        // Check if already cancelled
        let is_cancelled = *self.cancelled.lock().unwrap();
        eprintln!("[AUTH] Checking cancellation flag: {}", is_cancelled);

        if is_cancelled {
            eprintln!("[AUTH] Auth already cancelled, rejecting subsequent auth request");
            return Err(anyhow::anyhow!("Authentication was cancelled"));
        }

        eprintln!("[AUTH] Creating response storage and nested main loop");

        // Create response storage (shared between callback and main code)
        let response: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let response_clone = response.clone();

        // Create nested main loop for modal behavior
        let main_loop = glib::MainLoop::new(None, false);
        let main_loop_clone = main_loop.clone();

        let cancelled = self.cancelled.clone();

        eprintln!("[AUTH] Showing auth dialog on main thread");

        // Show dialog with callback
        unsafe {
            let window = &*(self.window_ptr as *const adw::ApplicationWindow);
            show_auth_dialog_with_callback(window, &request.prompt, request.hidden, move |result| {
                eprintln!("[AUTH] Dialog callback invoked");
                eprintln!("[AUTH] Result type: {}", if result.is_some() { "Some(text)" } else { "None (cancelled)" });

                if let Some(ref text) = result {
                    eprintln!("[AUTH] Response length: {} chars", text.len());
                }

                // Store response
                *response_clone.lock().unwrap() = Some(result.clone());
                eprintln!("[AUTH] Response stored in Arc<Mutex>");

                // Set cancelled flag if user cancelled
                if result.is_none() {
                    eprintln!("[AUTH] Setting cancelled flag to true");
                    *cancelled.lock().unwrap() = true;
                } else {
                    eprintln!("[AUTH] User provided input, not setting cancelled flag");
                }

                // Quit the nested event loop
                eprintln!("[AUTH] Quitting nested main loop");
                main_loop_clone.quit();
            });
        }

        eprintln!("[AUTH] Waiting for dialog response (main_loop.run())...");

        // Run nested event loop - blocks but processes GTK events
        main_loop.run();

        eprintln!("[AUTH] Main loop exited, extracting response");

        // Extract and return response
        let result = response.lock().unwrap().take();

        eprintln!("[AUTH] Response extracted from storage");

        match result {
            Some(Some(text)) => {
                eprintln!("[AUTH] Returning Ok with {} chars of text", text.len());
                Ok(text)
            }
            Some(None) => {
                eprintln!("[AUTH] Returning error: Authentication was cancelled");
                Err(anyhow::anyhow!("Authentication was cancelled"))
            }
            None => {
                eprintln!("[AUTH] ERROR: Dialog closed unexpectedly (no response stored)");
                Err(anyhow::anyhow!("Dialog closed unexpectedly"))
            }
        }
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

/// Show authentication dialog with a callback
fn show_auth_dialog_with_callback<F>(
    window: &adw::ApplicationWindow,
    prompt: &str,
    hidden: bool,
    callback: F,
) where
    F: FnOnce(Option<String>) + 'static,
{
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

    // Handle response with callback
    let entry_clone = entry.clone();
    let callback = std::cell::RefCell::new(Some(callback));
    let response_handled = std::cell::RefCell::new(false);

    dialog.connect_response(None, move |dialog, response| {
        // Ensure we only handle the response once
        if *response_handled.borrow() {
            eprintln!("Warning: Dialog response already handled, ignoring duplicate");
            return;
        }
        *response_handled.borrow_mut() = true;

        if response == "submit" {
            // Get the text from the appropriate widget type
            let text = if let Some(password_entry) = entry_clone.downcast_ref::<gtk4::PasswordEntry>() {
                password_entry.text().to_string()
            } else if let Some(text_entry) = entry_clone.downcast_ref::<gtk4::Entry>() {
                text_entry.text().to_string()
            } else {
                String::new()
            };

            eprintln!("Auth dialog submit with text length: {}", text.len());

            // Call callback with result
            if let Some(cb) = callback.borrow_mut().take() {
                if !text.is_empty() {
                    cb(Some(text));
                } else {
                    cb(None);
                }
            }
        } else {
            eprintln!("Auth dialog cancelled");
            // User cancelled
            if let Some(cb) = callback.borrow_mut().take() {
                cb(None);
            }
        }

        dialog.close();
    });

    dialog.present();
}

/// Internal function to show the auth dialog (legacy, kept for compatibility)
#[allow(dead_code)]
fn show_auth_dialog_internal(
    window: &adw::ApplicationWindow,
    prompt: &str,
    hidden: bool,
    auth_tx: Arc<Mutex<Option<std::sync::mpsc::Sender<Option<String>>>>>,
) {
    show_auth_dialog_with_callback(window, prompt, hidden, move |result| {
        let mut guard = auth_tx.lock().unwrap();
        if let Some(tx) = guard.take() {
            let _ = tx.send(result);
        }
    });
}
