// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Details panel - Profile details and controls

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation};
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;

use crate::models::profile_model::ProfileModel;
use super::profile_dialog;
use super::window::AppState;
use super::sidebar;

/// Update the details panel with a selected profile
pub fn update_with_profile(
    details_widget: &GtkBox,
    profile: &ProfileModel,
    state: Rc<AppState>,
    window: &adw::ApplicationWindow,
) {
    // Clear existing children
    while let Some(child) = details_widget.first_child() {
        details_widget.remove(&child);
    }

    // Create new content with profile details
    let content = create_profile_details(profile, state, window);
    details_widget.append(&content);
}

/// Create profile details view
fn create_profile_details(
    profile: &ProfileModel,
    state: Rc<AppState>,
    window: &adw::ApplicationWindow,
) -> GtkBox {
    let main_box = GtkBox::new(Orientation::Vertical, 0);
    main_box.set_vexpand(true);

    // Create scrolled window for details
    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let content_box = GtkBox::new(Orientation::Vertical, 24);
    content_box.set_margin_start(24);
    content_box.set_margin_end(24);
    content_box.set_margin_top(24);
    content_box.set_margin_bottom(24);

    // Profile header (centered)
    let header = create_header(profile);
    content_box.append(&header);

    // Status section (centered, with profile-specific status query)
    let status_section = create_status_section(profile, state.clone());
    content_box.append(&status_section);

    // Action buttons (centered)
    let buttons = create_action_buttons(profile, state.clone(), window);
    content_box.append(&buttons);

    // SSH Connection section
    let ssh_section = create_section("SSH Connection", &[
        ("Host", &profile.host()),
        ("Port", &profile.port().to_string()),
        ("User", &profile.user()),
    ]);
    content_box.append(&ssh_section);

    // Authentication section
    let auth_type = profile.auth_type();
    let key_path = profile.key_path();

    let auth_fields: Vec<(&str, &str)> = if auth_type == "SSH Key" {
        vec![("Auth Type", auth_type.as_str()), ("Key Path", key_path.as_str())]
    } else {
        vec![("Auth Type", auth_type.as_str())]
    };

    let auth_section = create_section("Authentication", &auth_fields);
    content_box.append(&auth_section);

    // Port Forwarding section
    let bind_address = profile.bind_address();
    let forwarding_text = if profile.local_port() > 0 {
        format!(
            "{}:{} → {}:{}",
            bind_address,
            profile.local_port(),
            profile.remote_host(),
            profile.remote_port()
        )
    } else {
        "Not configured".to_string()
    };

    let forward_section = create_section("Port Forwarding", &[
        ("Bind Address", &bind_address),
        ("Local Port", &profile.local_port().to_string()),
        ("Remote Host", &profile.remote_host()),
        ("Remote Port", &profile.remote_port().to_string()),
        ("Mapping", &forwarding_text),
    ]);
    content_box.append(&forward_section);

    scrolled.set_child(Some(&content_box));
    main_box.append(&scrolled);

    main_box
}

/// Create profile header with name and ID
fn create_header(profile: &ProfileModel) -> GtkBox {
    let header = GtkBox::new(Orientation::Vertical, 8);

    let name_label = Label::new(Some(&profile.name()));
    name_label.set_halign(gtk4::Align::Start);
    name_label.add_css_class("title-1");

    let id_label = Label::new(Some(&format!("ID: {}", profile.id())));
    id_label.set_halign(gtk4::Align::Start);
    id_label.add_css_class("dim-label");
    id_label.add_css_class("caption");

    header.append(&name_label);
    header.append(&id_label);

    header
}

/// Create a section with key-value pairs
fn create_section(title: &str, fields: &[(&str, &str)]) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(title)
        .build();

    for (key, value) in fields {
        let row = adw::ActionRow::builder()
            .title(*key)
            .build();

        let value_label = Label::new(Some(*value));
        value_label.add_css_class("dim-label");
        value_label.set_valign(gtk4::Align::Center);
        row.add_suffix(&value_label);

        group.add(&row);
    }

    group
}

/// Create status section
fn create_status_section(profile: &ProfileModel, state: Rc<AppState>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Status")
        .build();

    let status_row = adw::ActionRow::builder()
        .title("Connection")
        .build();

    // Status indicator
    let status_box = GtkBox::new(Orientation::Horizontal, 8);
    let status_icon = gtk4::Image::from_icon_name("media-playback-stop-symbolic");
    status_icon.add_css_class("dim-label");

    let status_label = Label::new(Some("Checking..."));
    status_label.add_css_class("dim-label");

    status_box.append(&status_icon);
    status_box.append(&status_label);
    status_row.add_suffix(&status_box);

    group.add(&status_row);

    // Query daemon for current status
    let profile = profile.clone();
    let status_icon = status_icon.clone();
    let status_label = status_label.clone();

    glib::MainContext::default().spawn_local(async move {
        if let Ok(status_text) = get_tunnel_status_text(&profile, &state).await {
            status_label.set_text(&status_text.0);
            status_icon.set_icon_name(Some(&status_text.1));
        } else {
            status_label.set_text("Unknown");
        }
    });

    group
}

/// Create action buttons
fn create_action_buttons(
    profile: &ProfileModel,
    state: Rc<AppState>,
    window: &adw::ApplicationWindow,
) -> GtkBox {
    let button_box = GtkBox::new(Orientation::Horizontal, 12);
    button_box.set_halign(gtk4::Align::Start);
    button_box.set_margin_top(12);

    let start_button = gtk4::Button::builder()
        .label("Start Tunnel")
        .build();
    start_button.add_css_class("suggested-action");

    // Wire up Start button
    {
        let profile = profile.clone();
        let state = state.clone();
        let window = window.clone();
        start_button.connect_clicked(move |button| {
            // Check if we need to show SSH key warning
            let inner_profile = match profile.profile() {
                Some(p) => p,
                None => {
                    eprintln!("✗ Profile data not available");
                    return;
                }
            };

            let profile = profile.clone();
            let state = state.clone();
            let window = window.clone();
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
                        button.set_sensitive(false);
                        button.set_label("Starting...");

                        let profile = profile.clone();
                        let state = state.clone();
                        let window = window.clone();
                        let button = button.clone();

                        glib::MainContext::default().spawn_local(async move {
                            let result = start_tunnel_async(&profile, &state).await;

                            button.set_sensitive(true);
                            button.set_label("Start Tunnel");

                            match result {
                                Ok(()) => {
                                    eprintln!("✓ Tunnel start request accepted by daemon");
                                }
                                Err(e) => {
                                    let error_msg = format!("Failed to start tunnel: {}", e);
                                    show_error_dialog(&window, &error_msg);
                                }
                            }
                        });
                    }
                });

                dialog.present();
            } else {
                // No warning needed - proceed directly
                button.set_sensitive(false);
                button.set_label("Starting...");

                let profile = profile.clone();
                let state = state.clone();
                let window = window.clone();
                let button = button.clone();

                glib::MainContext::default().spawn_local(async move {
                    let result = start_tunnel_async(&profile, &state).await;

                    button.set_sensitive(true);
                    button.set_label("Start Tunnel");

                    match result {
                        Ok(()) => {
                            eprintln!("✓ Tunnel start request accepted by daemon");
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
    }

    let stop_button = gtk4::Button::builder()
        .label("Stop Tunnel")
        .build();

    // Wire up Stop button
    {
        let profile = profile.clone();
        let state = state.clone();
        let window = window.clone();
        stop_button.connect_clicked(move |button| {
            // Disable button during operation
            button.set_sensitive(false);
            button.set_label("Stopping...");

            let profile = profile.clone();
            let state = state.clone();
            let window = window.clone();
            let button = button.clone();

            // Spawn async task to stop tunnel
            glib::MainContext::default().spawn_local(async move {
                let result = stop_tunnel_async(&profile, &state).await;

                // Re-enable button
                button.set_sensitive(true);
                button.set_label("Stop Tunnel");

                // Show result - only show errors, success will be reflected in status updates
                match result {
                    Ok(()) => {
                        eprintln!("✓ Tunnel stop request accepted by daemon");
                        // Status will update via SSE events
                    }
                    Err(e) => {
                        show_error_dialog(&window, &format!("Failed to stop tunnel: {}", e));
                    }
                }
            });
        });
    }

    let edit_button = gtk4::Button::builder()
        .label("Edit")
        .build();

    let delete_button = gtk4::Button::builder()
        .label("Delete")
        .build();
    delete_button.add_css_class("destructive-action");

    // Wire up Edit button
    {
        let window = window.clone();
        let profile = profile.clone();
        let state = state.clone();
        edit_button.connect_clicked(move |_| {
            profile_dialog::show_edit_profile_dialog(&window, &profile, state.clone());
        });
    }

    // Wire up Delete button
    {
        let window = window.clone();
        let profile = profile.clone();
        let state = state.clone();
        delete_button.connect_clicked(move |_| {
            show_delete_confirmation(&window, &profile, state.clone());
        });
    }

    button_box.append(&start_button);
    button_box.append(&stop_button);
    button_box.append(&edit_button);
    button_box.append(&delete_button);

    button_box
}

/// Show delete confirmation dialog
fn show_delete_confirmation(
    parent: &adw::ApplicationWindow,
    profile: &ProfileModel,
    state: Rc<AppState>,
) {
    let dialog = adw::MessageDialog::builder()
        .transient_for(parent)
        .heading("Delete Profile")
        .body(&format!(
            "Are you sure you want to delete the profile '{}'?\n\nThis action cannot be undone.",
            profile.name()
        ))
        .build();

    dialog.add_response("cancel", "Cancel");
    dialog.add_response("delete", "Delete");
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    let profile = profile.clone();
    dialog.connect_response(None, move |_, response| {
        if response == "delete" {
            if let Err(e) = delete_profile(&profile) {
                eprintln!("Failed to delete profile: {}", e);
                // TODO: Show error dialog
            } else {
                // Reload profile list after successful deletion
                sidebar::reload_profile_list(state.clone());
            }
        }
    });

    dialog.present();
}

/// Delete a profile from disk
fn delete_profile(profile: &ProfileModel) -> anyhow::Result<()> {
    use crate::utils::profiles;

    if let Some(inner_profile) = profile.profile() {
        let profiles_dir = profiles::get_profiles_dir()?;
        let filename = format!("{}.toml", inner_profile.metadata.id);
        let path = profiles_dir.join(filename);

        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }

    Ok(())
}

/// Create placeholder widget for when no profile is selected
pub fn create_placeholder() -> adw::StatusPage {
    adw::StatusPage::builder()
        .icon_name("document-properties-symbolic")
        .title("Select a Profile")
        .description("Choose a profile from the list to view its details")
        .vexpand(true)
        .build()
}

/// Async function to start a tunnel; SSE listener handles auth/status updates
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

/// Async function to stop a tunnel using shared helper
async fn stop_tunnel_async(profile: &ProfileModel, state: &Rc<AppState>) -> anyhow::Result<()> {
    use ssh_tunnel_common::{create_daemon_client, stop_tunnel};

    // Get profile data
    let inner_profile = profile
        .profile()
        .ok_or_else(|| anyhow::anyhow!("Profile data not available"))?;
    let tunnel_id = inner_profile.metadata.id;

    // Get daemon config from state
    let daemon_config = state
        .daemon_client
        .borrow()
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Daemon client not available"))?
        .config
        .clone();

    // Create HTTP client
    let client = create_daemon_client(&daemon_config)?;

    // Use the shared stop helper
    stop_tunnel(&client, &daemon_config, tunnel_id).await?;

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

/// Get tunnel status text and icon for a profile
/// Returns (status_text, icon_name)
async fn get_tunnel_status_text(
    profile: &ProfileModel,
    state: &Rc<AppState>,
) -> anyhow::Result<(String, String)> {
    // Get profile ID
    let inner_profile = profile
        .profile()
        .ok_or_else(|| anyhow::anyhow!("Profile data not available"))?;
    let profile_id = inner_profile.metadata.id;

    // Get daemon client
    let daemon_client = state
        .daemon_client
        .borrow()
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Daemon client not available"))?
        .clone();

    // Query tunnel status
    match daemon_client.get_tunnel_status(profile_id).await? {
        Some(status_response) => {
            use ssh_tunnel_common::TunnelStatus;

            let (text, icon) = match status_response.status {
                TunnelStatus::NotConnected => ("Not Connected", "media-playback-stop-symbolic"),
                TunnelStatus::Connecting => ("Connecting...", "emblem-synchronizing-symbolic"),
                TunnelStatus::WaitingForAuth => ("Auth Required", "dialog-question-symbolic"),
                TunnelStatus::Connected => ("Connected", "network-transmit-receive-symbolic"),
                TunnelStatus::Disconnecting => ("Disconnecting...", "emblem-synchronizing-symbolic"),
                TunnelStatus::Disconnected => ("Disconnected", "network-offline-symbolic"),
                TunnelStatus::Reconnecting => ("Reconnecting...", "emblem-synchronizing-symbolic"),
                TunnelStatus::Failed(ref err) => {
                    return Ok((format!("Failed: {}", err), "dialog-error-symbolic".to_string()))
                }
            };

            Ok((text.to_string(), icon.to_string()))
        }
        None => {
            // Tunnel not active in daemon
            Ok(("Stopped".to_string(), "media-playback-stop-symbolic".to_string()))
        }
    }
}
