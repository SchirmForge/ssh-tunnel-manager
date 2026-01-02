// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Profile details page (shown when a profile is selected from the list)

use gtk4::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;
use std::time::{Duration, Instant};
use uuid::Uuid;

use super::window::AppState;
use crate::models::profile_model::ProfileModel;
use ssh_tunnel_common::{ForwardingType, PasswordStorage, TunnelStatus};

/// Create the profile details view
pub fn create(state: Rc<AppState>, profile: &ProfileModel) -> adw::NavigationPage {
    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // Create header bar with back button
    let header = adw::HeaderBar::new();
    header.set_show_back_button(true); // Show back button to return to list
    header.add_css_class("flat"); // Match content background
    content_box.append(&header);

    // Connection status banner at top (full-width, sticky)
    let status_banner = create_status_banner(&profile);
    content_box.append(&status_banner);

    // Store banner reference in state for SSE updates
    state.profile_details_banner.replace(Some(status_banner.clone()));

    // Create scrolled window for content
    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);

    // Create main content box with centered layout
    let main_content = gtk4::Box::new(gtk4::Orientation::Vertical, 24);
    main_content.set_margin_top(24);
    main_content.set_margin_bottom(24);
    main_content.set_margin_start(24);
    main_content.set_margin_end(24);

    // Profile summary (4 key fields)
    let summary_group = create_summary_group(&profile);
    main_content.append(&summary_group);

    // Action buttons (Start/Stop/Edit/Delete)
    // Get window from state for dialogs
    let window = state.window.borrow().as_ref().cloned().expect("Window not available");
    let (actions_box, start_btn, stop_btn) = create_action_buttons(state.clone(), profile, &window);
    main_content.append(&actions_box);

    // Store button references in state for SSE updates
    state.profile_details_start_btn.replace(Some(start_btn));
    state.profile_details_stop_btn.replace(Some(stop_btn));

    // Full details in expandable section (below buttons)
    let details_expander = create_details_expander(&profile);
    main_content.append(&details_expander);

    scrolled.set_child(Some(&main_content));

    // Create clamp for centered content
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(800);
    clamp.set_child(Some(&scrolled));

    content_box.append(&clamp);

    // Create navigation page
    let page = adw::NavigationPage::builder()
        .title(&profile.name())
        .child(&content_box)
        .build();

    // Initialize tunnel status when page is created
    let state_clone = state.clone();
    let profile_clone = profile.clone();
    glib::MainContext::default().spawn_local(async move {
        if let Some(prof) = profile_clone.profile() {
            let profile_id = prof.metadata.id;

            // Query current tunnel status from daemon
            if let Some(client) = state_clone.daemon_client.borrow().as_ref() {
                match client.get_tunnel_status(profile_id).await {
                    Ok(Some(status_response)) => {
                        // Update UI with current status
                        update_tunnel_status(&state_clone, status_response.status);
                    }
                    Ok(None) => {
                        // Tunnel not running
                        update_tunnel_status(&state_clone, ssh_tunnel_common::TunnelStatus::NotConnected);
                    }
                    Err(e) => {
                        eprintln!("Failed to get tunnel status: {}", e);
                        update_tunnel_status(&state_clone, ssh_tunnel_common::TunnelStatus::NotConnected);
                    }
                }
            } else {
                eprintln!("Daemon client not available");
                update_tunnel_status(&state_clone, ssh_tunnel_common::TunnelStatus::NotConnected);
            }
        }
    });

    page
}

/// Create connection status banner (informational only, no action button)
fn create_status_banner(_profile: &ProfileModel) -> adw::Banner {
    let banner = adw::Banner::new("Not connected");
    banner.set_revealed(true);
    banner.add_css_class("info");

    // Store reference for updates (we'll update this from status polling)
    banner.set_title("Checking connection status...");

    banner
}

/// Create profile summary group (4 key fields)
fn create_summary_group(profile: &ProfileModel) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title("Connection Summary");

    // Host row
    let host_row = adw::ActionRow::new();
    host_row.set_title("Host");
    host_row.set_subtitle(&format!("{}@{}", profile.user(), profile.host()));
    let host_icon = gtk4::Image::from_icon_name("network-server-symbolic");
    host_row.add_prefix(&host_icon);
    group.add(&host_row);

    // SSH Port row
    let port_row = adw::ActionRow::new();
    port_row.set_title("SSH Port");
    port_row.set_subtitle(&profile.port().to_string());
    let port_icon = gtk4::Image::from_icon_name("preferences-system-symbolic");
    port_row.add_prefix(&port_icon);
    group.add(&port_row);

    // Tunnel configuration row with detailed description
    if let Some(prof) = profile.profile() {
        let tunnel_row = adw::ActionRow::new();
        tunnel_row.set_title("Tunnel Configuration");

        let tunnel_description = ssh_tunnel_common::format_tunnel_description(&prof.forwarding);

        tunnel_row.set_subtitle(&tunnel_description);
        let tunnel_icon = gtk4::Image::from_icon_name("network-transmit-receive-symbolic");
        tunnel_row.add_prefix(&tunnel_icon);
        group.add(&tunnel_row);

        // Forwarding type row
        let type_row = adw::ActionRow::new();
        type_row.set_title("Forwarding Type");
        let forwarding_type = match prof.forwarding.forwarding_type {
            ForwardingType::Local => "Local Port Forwarding",
            ForwardingType::Remote => "Remote Port Forwarding",
            ForwardingType::Dynamic => "Dynamic SOCKS Proxy",
        };
        type_row.set_subtitle(forwarding_type);
        let type_icon = gtk4::Image::from_icon_name("emblem-system-symbolic");
        type_row.add_prefix(&type_icon);
        group.add(&type_row);
    }

    group
}

/// Create expandable details section
fn create_details_expander(profile: &ProfileModel) -> adw::ExpanderRow {
    let expander = adw::ExpanderRow::new();
    expander.set_title("Full Profile Details");
    expander.set_subtitle("View all configuration options");

    if let Some(prof) = profile.profile() {
        // Authentication method
        let auth_row = adw::ActionRow::new();
        auth_row.set_title("Authentication");
        let auth_method = match prof.connection.auth_type {
            ssh_tunnel_common::AuthType::Password => "Password",
            ssh_tunnel_common::AuthType::PasswordWith2FA => "Password with 2FA",
            ssh_tunnel_common::AuthType::Key => "SSH Key",
        };
        auth_row.set_subtitle(auth_method);
        expander.add_row(&auth_row);

        // Key path (if using key auth)
        if prof.connection.auth_type == ssh_tunnel_common::AuthType::Key {
            if let Some(key_path) = &prof.connection.key_path {
                let key_row = adw::ActionRow::new();
                key_row.set_title("SSH Key Path");
                key_row.set_subtitle(&key_path.display().to_string());
                expander.add_row(&key_row);
            }
        }

        // Password stored
        let password_row = adw::ActionRow::new();
        password_row.set_title("Password Stored");
        password_row.set_subtitle(if prof.connection.password_storage == PasswordStorage::Keychain { "Yes (in keyring)" } else { "No" });
        expander.add_row(&password_row);

        // Keepalive interval
        let keepalive_row = adw::ActionRow::new();
        keepalive_row.set_title("Keepalive Interval");
        keepalive_row.set_subtitle(&format!("{} seconds", prof.options.keepalive_interval));
        expander.add_row(&keepalive_row);

        // Auto reconnect
        let reconnect_row = adw::ActionRow::new();
        reconnect_row.set_title("Auto Reconnect");
        reconnect_row.set_subtitle(if prof.options.auto_reconnect { "Enabled" } else { "Disabled" });
        expander.add_row(&reconnect_row);

        // Profile ID
        let id_row = adw::ActionRow::new();
        id_row.set_title("Profile ID");
        id_row.set_subtitle(&prof.metadata.id.to_string());
        expander.add_row(&id_row);

        // Created at
        let created_row = adw::ActionRow::new();
        created_row.set_title("Created");
        created_row.set_subtitle(&prof.metadata.created_at.format("%Y-%m-%d %H:%M:%S").to_string());
        expander.add_row(&created_row);
    }

    expander
}

/// Create action buttons (Start/Stop/Edit/Delete)
/// Returns (button_box, start_button, stop_button) for storing references in AppState
fn create_action_buttons(state: Rc<AppState>, profile: &ProfileModel, window: &adw::ApplicationWindow) -> (gtk4::Box, gtk4::Button, gtk4::Button) {
    let button_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    button_box.set_halign(gtk4::Align::Center);
    button_box.set_margin_top(24);

    // Start button (primary action)
    let start_button = gtk4::Button::with_label("Start");
    start_button.add_css_class("suggested-action");
    start_button.add_css_class("pill");
    start_button.set_size_request(120, -1);

    let profile_clone = profile.clone();
    let state_clone = state.clone();
    let window_clone = window.clone();
    start_button.connect_clicked(move |button| {
        // Disable button immediately to prevent double-clicks
        if !button.is_sensitive() {
            tracing::debug!("Start button already clicked, ignoring");
            return;
        }
        button.set_sensitive(false);

        // Check if we need to show SSH key warning
        let inner_profile = match profile_clone.profile() {
            Some(p) => p,
            None => {
                eprintln!("✗ Profile data not available");
                button.set_sensitive(true);
                return;
            }
        };

        let profile = profile_clone.clone();
        let state = state_clone.clone();
        let window = window_clone.clone();
        let button = button.clone();

        // Spawn async task to check for warning and handle start
        glib::MainContext::default().spawn_local(async move {
            // Check if daemon client needs SSH key warning (async)
            let warning_message = if let Some(client) = state.daemon_client.borrow().as_ref() {
                client.needs_ssh_key_warning(&inner_profile).await
            } else {
                None
            };

            if let Some(warning_msg) = warning_message {
            // Show warning dialog with Continue/Cancel
            let dialog = adw::MessageDialog::builder()
                .transient_for(&window)
                .heading("SSH Key Setup Required")
                .body(&warning_msg)
                .build();

            // Add checkbox to dialog for "Don't show this again"
            let checkbox = gtk4::CheckButton::with_label("Don't show this message again");
            checkbox.set_margin_top(12);
            checkbox.set_margin_bottom(12);
            dialog.set_extra_child(Some(&checkbox));

            dialog.add_response("cancel", "Cancel");
            dialog.add_response("continue", "Continue");
            dialog.set_response_appearance("continue", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("continue"));
            dialog.set_close_response("cancel");

            let profile = profile.clone();
            let state = state.clone();
            let window = window.clone();
            let button = button.clone();

            dialog.connect_response(None, move |dialog_ref, response| {
                if response == "continue" {
                    // Check if user wants to skip future warnings
                    if let Some(extra) = dialog_ref.extra_child() {
                        if let Some(checkbox) = extra.downcast_ref::<gtk4::CheckButton>() {
                            if checkbox.is_active() {
                                // Save preference to config file
                                let state_for_save = state.clone();
                                glib::MainContext::default().spawn_local(async move {
                                    if let Err(e) = ssh_tunnel_gui_core::save_skip_ssh_warning_preference(true).await {
                                        tracing::warn!("Failed to save skip SSH warning preference: {}", e);
                                    }
                                    // Update daemon client config in memory
                                    if let Some(client) = state_for_save.daemon_client.borrow_mut().as_mut() {
                                        client.set_skip_ssh_warning(true);
                                    }
                                });
                            }
                        }
                    }

                    // User clicked Continue - proceed with starting tunnel
                    // Button already disabled in click handler

                    let profile = profile.clone();
                    let state = state.clone();
                    let window = window.clone();
                    let button = button.clone();

                    glib::MainContext::default().spawn_local(async move {
                        let result = start_tunnel_async(&profile, &state).await;

                        button.set_sensitive(true);

                        match result {
                            Ok(()) => {
                                eprintln!("✓ Tunnel start request accepted by daemon");

                                // Start per-tunnel polling for guaranteed delivery
                                if let Some(p) = profile.profile() {
                                    let tunnel_id = p.metadata.id;
                                    let state_clone = state.clone();

                                    glib::MainContext::default().spawn_local(async move {
                                        poll_tunnel_until_terminal(tunnel_id, &state_clone).await;
                                    });
                                }
                            }
                            Err(e) => {
                                let error_msg = format!("Failed to start tunnel: {}", e);
                                show_error_dialog(&window, &error_msg);
                            }
                        }
                    });
                } else {
                    // User cancelled - re-enable button
                    button.set_sensitive(true);
                }
            });

            dialog.present();
        } else {
            // No warning needed - proceed directly
            // Button already disabled at start of click handler

            let profile = profile.clone();
            let state = state.clone();
            let window = window.clone();
            let button = button.clone();

            glib::MainContext::default().spawn_local(async move {
                let result = start_tunnel_async(&profile, &state).await;

                button.set_sensitive(true);

                match result {
                    Ok(()) => {
                        eprintln!("✓ Tunnel start request accepted by daemon");

                        // Start per-tunnel polling for guaranteed delivery
                        if let Some(p) = profile.profile() {
                            let tunnel_id = p.metadata.id;
                            let state_clone = state.clone();

                            glib::MainContext::default().spawn_local(async move {
                                poll_tunnel_until_terminal(tunnel_id, &state_clone).await;
                            });
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to start tunnel: {}", e);
                        show_error_dialog(&window, &error_msg);
                    }
                }
            });
            }
        });
    });
    button_box.append(&start_button);

    // Stop button (destructive action)
    let stop_button = gtk4::Button::with_label("Stop");
    stop_button.add_css_class("destructive-action");
    stop_button.add_css_class("pill");
    stop_button.set_size_request(120, -1);
    stop_button.set_sensitive(false); // Disabled until connected

    let profile_clone = profile.clone();
    let state_clone = state.clone();
    stop_button.connect_clicked(move |button| {
        eprintln!("Stopping tunnel for profile: {}", profile_clone.name());

        if let Some(prof) = profile_clone.profile() {
            let tunnel_id = prof.metadata.id;
            let state = state_clone.clone();
            let button = button.clone();

            // Disable button during operation
            button.set_sensitive(false);

            glib::MainContext::default().spawn_local(async move {
                // Use the shared stop helper
                use ssh_tunnel_common::{create_daemon_client, stop_tunnel};

                // Get daemon config
                let daemon_config = match state.daemon_client.borrow().as_ref() {
                    Some(client) => client.config.clone(),
                    None => {
                        eprintln!("✗ Daemon client not available");
                        button.set_sensitive(true);
                        return;
                    }
                };

                // Create HTTP client
                let client = match create_daemon_client(&daemon_config) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("✗ Failed to create daemon client: {}", e);
                        button.set_sensitive(true);
                        return;
                    }
                };

                // Stop tunnel
                match stop_tunnel(&client, &daemon_config, tunnel_id).await {
                    Ok(_) => {
                        eprintln!("✓ Tunnel stopped successfully");
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to stop tunnel: {}", e);
                    }
                }

                // Re-enable button
                button.set_sensitive(true);
            });
        }
    });
    button_box.append(&stop_button);

    // Edit button
    let edit_button = gtk4::Button::with_label("Edit");
    edit_button.add_css_class("pill");
    edit_button.set_size_request(120, -1);

    let profile_clone = profile.clone();
    let state_clone = state.clone();
    edit_button.connect_clicked(move |_| {
        eprintln!("Edit profile: {}", profile_clone.name());

        // Get window for dialog parent
        if let Some(window) = state_clone.window.borrow().as_ref() {
            super::profile_dialog::show_edit_profile_dialog(window, &profile_clone, state_clone.clone());
        } else {
            eprintln!("Cannot edit: window not available");
        }
    });
    button_box.append(&edit_button);

    // Delete button
    let delete_button = gtk4::Button::with_label("Delete");
    delete_button.add_css_class("pill");
    delete_button.set_size_request(120, -1);

    let profile_clone = profile.clone();
    let state_clone = state.clone();
    delete_button.connect_clicked(move |_| {
        let profile_name = profile_clone.name();
        eprintln!("Delete profile: {}", profile_name);

        // Get profile ID
        let profile_id = match profile_clone.profile() {
            Some(prof) => prof.metadata.id,
            None => {
                eprintln!("Cannot delete: profile data not available");
                return;
            }
        };

        // Get window for dialog parent
        let window = match state_clone.window.borrow().as_ref() {
            Some(w) => w.clone(),
            None => {
                eprintln!("Cannot delete: window not available");
                return;
            }
        };

        // Show confirmation dialog
        let dialog = adw::MessageDialog::builder()
            .transient_for(&window)
            .heading("Delete Profile?")
            .body(&format!("Are you sure you want to delete '{}'?\n\nThis action cannot be undone.", profile_name))
            .build();

        dialog.add_response("cancel", "Cancel");
        dialog.add_response("delete", "Delete");
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        // Handle response
        let state_for_response = state_clone.clone();
        let nav_view = state_clone.nav_view.borrow().clone();
        dialog.connect_response(None, move |_, response| {
            if response == "delete" {
                // Delete the profile from disk using common function
                match ssh_tunnel_common::delete_profile_by_id(&profile_id) {
                    Ok(_) => {
                        eprintln!("✓ Profile deleted successfully");

                        // Navigate back to profiles list
                        if let Some(nav_view) = nav_view.as_ref() {
                            nav_view.pop();
                        }

                        // Refresh the profiles list
                        if let Some(list_box) = state_for_response.profile_list.borrow().as_ref() {
                            super::profiles_list::populate_profiles(list_box, state_for_response.clone());
                        }
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to delete profile: {}", e);
                        // TODO: Show error dialog
                    }
                }
            }
        });

        dialog.present();
    });
    button_box.append(&delete_button);

    (button_box, start_button, stop_button)
}

/// Update the profile details UI based on tunnel status
/// This is called by the SSE event handler when tunnel status changes
pub fn update_tunnel_status(state: &AppState, status: TunnelStatus) {
    tracing::debug!("profile_details::update_tunnel_status called with status: {:?}", status);

    // Update status banner
    if let Some(banner) = state.profile_details_banner.borrow().as_ref() {
        tracing::debug!("Found banner widget, updating...");

        let (message, css_class) = match &status {
            TunnelStatus::NotConnected => ("Not connected", "info"),
            TunnelStatus::Connecting => ("Connecting...", "info"),
            TunnelStatus::WaitingForAuth => ("Waiting for authentication", "warning"),
            TunnelStatus::Connected => ("Connected", "success"),
            TunnelStatus::Disconnecting => ("Disconnecting...", "info"),
            TunnelStatus::Disconnected => ("Disconnected", "info"),
            TunnelStatus::Reconnecting => ("Reconnecting...", "warning"),
            TunnelStatus::Failed(reason) => {
                tracing::debug!("Setting Failed status in banner: {}", reason);
                banner.set_title(&format!("Connection failed: {}", reason));
                banner.remove_css_class("info");
                banner.remove_css_class("success");
                banner.remove_css_class("warning");
                banner.add_css_class("error");
                banner.set_revealed(true);
                tracing::debug!("Banner updated with error styling");

                // Enable/disable buttons for failed state
                if let Some(start_btn) = state.profile_details_start_btn.borrow().as_ref() {
                    start_btn.set_sensitive(true);
                    tracing::debug!("Start button enabled");
                }
                if let Some(stop_btn) = state.profile_details_stop_btn.borrow().as_ref() {
                    stop_btn.set_sensitive(false);
                    tracing::debug!("Stop button disabled");
                }
                tracing::debug!("profile_details::update_tunnel_status completed (Failed)");
                return;
            }
        };

        banner.set_title(message);
        banner.remove_css_class("info");
        banner.remove_css_class("success");
        banner.remove_css_class("warning");
        banner.remove_css_class("error");
        banner.add_css_class(css_class);
        banner.set_revealed(true);
    }

    // Update button states based on status
    if let (Some(start_btn), Some(stop_btn)) = (
        state.profile_details_start_btn.borrow().as_ref(),
        state.profile_details_stop_btn.borrow().as_ref(),
    ) {
        match status {
            TunnelStatus::NotConnected | TunnelStatus::Disconnected | TunnelStatus::Failed(_) => {
                start_btn.set_sensitive(true);
                stop_btn.set_sensitive(false);
            }
            TunnelStatus::Connecting | TunnelStatus::WaitingForAuth => {
                start_btn.set_sensitive(false);
                stop_btn.set_sensitive(true); // Allow stopping during connection attempt
            }
            TunnelStatus::Connected => {
                start_btn.set_sensitive(false);
                stop_btn.set_sensitive(true);
            }
            TunnelStatus::Disconnecting | TunnelStatus::Reconnecting => {
                start_btn.set_sensitive(false);
                stop_btn.set_sensitive(false); // Disable both during transition
            }
        }
    }
}

/// Async function to start a tunnel
async fn start_tunnel_async(profile: &ProfileModel, state: &Rc<AppState>) -> anyhow::Result<()> {
    // Get profile data
    let inner_profile = profile
        .profile()
        .ok_or_else(|| anyhow::anyhow!("Profile data not available"))?;

    // Send start request; daemon SSE events will drive auth prompts and UI updates
    let daemon_client = state
        .daemon_client
        .borrow()
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Daemon client not available"))?
        .clone();

    daemon_client.start_tunnel(&inner_profile).await?;
    Ok(())
}

/// Show an error dialog
fn show_error_dialog(parent: &impl IsA<gtk4::Window>, message: &str) {
    let dialog = adw::MessageDialog::builder()
        .transient_for(parent)
        .heading("Error")
        .body(message)
        .build();

    dialog.add_response("ok", "OK");
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("ok");

    dialog.present();
}

/// Poll tunnel status after POST /start until terminal state is reached
/// This provides guaranteed delivery of status changes and auth prompts,
/// even if SSE events are lost during connection establishment.
async fn poll_tunnel_until_terminal(tunnel_id: Uuid, state: &Rc<AppState>) {
    let poll_interval = Duration::from_millis(500);
    let max_duration = Duration::from_secs(120); // 2 minute timeout
    let start_time = Instant::now();

    tracing::info!("Starting per-tunnel polling for tunnel {}", tunnel_id);

    loop {
        // Check if we've exceeded max duration
        if start_time.elapsed() > max_duration {
            tracing::warn!("Polling timeout for tunnel {} after {} seconds", tunnel_id, max_duration.as_secs());
            break;
        }

        // Query status via REST
        // Clone the client to avoid holding RefCell borrow across await
        let client = state.daemon_client.borrow().clone();
        if let Some(client) = client {
            match client.get_tunnel_status(tunnel_id).await {
                Ok(Some(response)) => {
                    // Update AppCore state
                    {
                        let mut core = state.core.borrow_mut();
                        core.tunnel_statuses.insert(tunnel_id, response.status.clone());
                    }

                    // Update UI
                    if let Some(list_box) = state.profile_list.borrow().as_ref() {
                        super::profiles_list::update_profile_status(list_box, tunnel_id, response.status.clone());
                    }

                    // Handle pending auth
                    if let Some(request) = response.pending_auth {
                        tracing::info!("Poll: Found pending auth for tunnel {} - queueing", tunnel_id);
                        if let Some(window) = state.window.borrow().as_ref() {
                            super::auth_dialog::handle_auth_request(window, request, state.clone());
                        }
                    }

                    // Check if terminal state reached
                    match &response.status {
                        TunnelStatus::Connected | TunnelStatus::Failed(_) | TunnelStatus::Disconnected => {
                            tracing::info!("Poll: Tunnel {} reached terminal state: {:?} - stopping poll", tunnel_id, response.status);

                            // Trigger status change handler on next main loop iteration
                            // to avoid race conditions with dialog callbacks
                            let state_clone = state.clone();
                            let status_clone = response.status.clone();
                            glib::idle_add_local_once(move || {
                                super::event_handler::handle_status_changed(&state_clone, tunnel_id, status_clone);
                            });
                            break;
                        }
                        TunnelStatus::NotConnected => {
                            tracing::info!("Poll: Tunnel {} not connected - stopping poll", tunnel_id);
                            break;
                        }
                        _ => {
                            // Transitional state - keep polling
                            tracing::debug!("Poll: Tunnel {} status: {:?}", tunnel_id, response.status);
                        }
                    }
                }
                Ok(None) => {
                    tracing::warn!("Poll: Tunnel {} not found (404) - stopping poll", tunnel_id);
                    break;
                }
                Err(e) => {
                    tracing::warn!("Poll: Failed to get status for tunnel {}: {}", tunnel_id, e);
                    // Continue polling despite errors
                }
            }
        }

        // Wait before next poll
        tokio::time::sleep(poll_interval).await;
    }

    tracing::info!("Stopped polling for tunnel {}", tunnel_id);
}
