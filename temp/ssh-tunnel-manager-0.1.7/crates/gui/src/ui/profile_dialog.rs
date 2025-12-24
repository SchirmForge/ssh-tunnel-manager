// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Profile editor dialog - Create and edit profiles

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Entry, Orientation, SpinButton};
use libadwaita as adw;
use adw::prelude::*;
use uuid::Uuid;
use std::rc::Rc;

use crate::models::profile_model::ProfileModel;
use crate::ui::window::AppState;
use ssh_tunnel_common::config::{
    Profile, ProfileMetadata, ConnectionConfig, ForwardingConfig, PasswordStorage, TunnelOptions,
};
use ssh_tunnel_common::types::{AuthType, ForwardingType};

/// Show profile editor dialog for creating a new profile
pub fn show_new_profile_dialog(parent: &impl IsA<gtk4::Window>, state: Rc<AppState>) {
    let dialog = create_dialog(parent, "New Profile", None, state);
    dialog.present();
}

/// Show profile editor dialog for editing an existing profile
pub fn show_edit_profile_dialog(
    parent: &impl IsA<gtk4::Window>,
    profile: &ProfileModel,
    state: Rc<AppState>,
) {
    let dialog = create_dialog(parent, "Edit Profile", Some(profile), state);
    dialog.present();
}

/// Create the profile editor dialog
fn create_dialog(
    parent: &impl IsA<gtk4::Window>,
    title: &str,
    profile: Option<&ProfileModel>,
    state: Rc<AppState>,
) -> adw::Window {
    let dialog = adw::Window::builder()
        .modal(true)
        .transient_for(parent)
        .default_width(500)
        .default_height(600)
        .title(title)
        .build();

    // Add ESC key handler to close dialog
    {
        let dialog_clone = dialog.clone();
        let key_controller = gtk4::EventControllerKey::new();
        key_controller.connect_key_pressed(move |_controller, key, _code, _modifier| {
            if key == gtk4::gdk::Key::Escape {
                dialog_clone.close();
                gtk4::glib::Propagation::Stop
            } else {
                gtk4::glib::Propagation::Proceed
            }
        });
        dialog.add_controller(key_controller);
    }

    // Create toolbar view
    let toolbar_view = adw::ToolbarView::new();

    // Header bar
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);

    // Cancel button
    let cancel_button = gtk4::Button::builder()
        .label("Cancel")
        .build();

    // Save button
    let save_button = gtk4::Button::builder()
        .label("Save")
        .build();
    save_button.add_css_class("suggested-action");

    header.pack_start(&cancel_button);
    header.pack_end(&save_button);

    toolbar_view.add_top_bar(&header);

    // Create scrolled content area
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

    // Basic fields group
    let basic_group = adw::PreferencesGroup::builder()
        .title("Basic Information")
        .description("Required connection details")
        .build();

    let name_entry = Entry::builder()
        .placeholder_text("My SSH Tunnel")
        .text("My SSH Tunnel")
        .build();
    let name_row = adw::ActionRow::builder()
        .title("Profile Name")
        .build();
    name_row.add_suffix(&name_entry);

    let host_entry = Entry::builder()
        .placeholder_text("example.com")
        .build();
    let host_row = adw::ActionRow::builder()
        .title("SSH Host")
        .subtitle("Remote server address")
        .build();
    host_row.add_suffix(&host_entry);

    let port_spin = SpinButton::with_range(1.0, 65535.0, 1.0);
    port_spin.set_value(22.0);
    let port_row = adw::ActionRow::builder()
        .title("SSH Port")
        .subtitle("Remote SSH port (default: 22)")
        .build();
    port_row.add_suffix(&port_spin);

    let user_entry = Entry::builder()
        .placeholder_text("username")
        .build();
    let user_row = adw::ActionRow::builder()
        .title("SSH User")
        .subtitle("Username for SSH connection")
        .build();
    user_row.add_suffix(&user_entry);

    basic_group.add(&name_row);
    basic_group.add(&host_row);
    basic_group.add(&port_row);
    basic_group.add(&user_row);
    content_box.append(&basic_group);

    // Authentication section - visible by default
    let auth_group = adw::PreferencesGroup::builder()
        .title("Authentication")
        .description("SSH authentication configuration")
        .build();

    // SSH Key switch
    let key_switch = gtk4::Switch::new();
    key_switch.set_active(false);
    key_switch.set_valign(gtk4::Align::Center);
    let key_row = adw::ActionRow::builder()
        .title("Use SSH Key")
        .subtitle("Authenticate with SSH private key")
        .activatable(true)
        .build();
    key_row.add_suffix(&key_switch);
    key_row.set_activatable_widget(Some(&key_switch));

    // SSH Key path
    let key_path_box = GtkBox::new(Orientation::Horizontal, 6);
    let key_path_entry = Entry::builder()
        .placeholder_text("~/.ssh/id_ed25519")
        .text("~/.ssh/id_ed25519")
        .hexpand(true)
        .sensitive(false)
        .build();

    let browse_button = gtk4::Button::builder()
        .icon_name("document-open-symbolic")
        .tooltip_text("Browse for SSH key")
        .sensitive(false)
        .build();

    key_path_box.append(&key_path_entry);
    key_path_box.append(&browse_button);

    let key_path_row = adw::ActionRow::builder()
        .title("Key Path")
        .subtitle("Path to SSH private key file")
        .build();
    key_path_row.add_suffix(&key_path_box);

    // Store in keychain switch (moved before password entry)
    let store_keychain_switch = gtk4::Switch::new();
    store_keychain_switch.set_active(false);
    store_keychain_switch.set_valign(gtk4::Align::Center);
    store_keychain_switch.set_sensitive(false);
    let store_keychain_row = adw::ActionRow::builder()
        .title("Store Passphrase in Keychain")
        .subtitle("Save passphrase in system keychain for automatic retrieval")
        .activatable(true)
        .build();
    store_keychain_row.add_suffix(&store_keychain_switch);
    store_keychain_row.set_activatable_widget(Some(&store_keychain_switch));

    // Key password (only shown when keychain is enabled)
    let key_password_entry = gtk4::PasswordEntry::builder()
        .show_peek_icon(true)
        .sensitive(false)
        .build();
    let key_password_row = adw::ActionRow::builder()
        .title("Key Passphrase")
        .subtitle("Enter passphrase for encrypted SSH key")
        .visible(false)
        .build();
    key_password_row.add_suffix(&key_password_entry);

    auth_group.add(&key_row);
    auth_group.add(&key_path_row);
    auth_group.add(&store_keychain_row);
    auth_group.add(&key_password_row);
    content_box.append(&auth_group);

    // Wire up key switch to enable/disable key fields
    {
        let key_path_entry = key_path_entry.clone();
        let browse_button = browse_button.clone();
        let store_keychain_switch = store_keychain_switch.clone();
        let key_password_row = key_password_row.clone();
        key_switch.connect_active_notify(move |switch| {
            let active = switch.is_active();
            key_path_entry.set_sensitive(active);
            browse_button.set_sensitive(active);
            store_keychain_switch.set_sensitive(active);
            if !active {
                store_keychain_switch.set_active(false);
                key_password_row.set_visible(false);
            }
        });
    }

    // Wire up keychain switch to show/hide password entry
    {
        let key_password_row = key_password_row.clone();
        let key_password_entry = key_password_entry.clone();
        store_keychain_switch.connect_active_notify(move |switch| {
            let active = switch.is_active();
            key_password_row.set_visible(active);
            key_password_entry.set_sensitive(active);
            if !active {
                key_password_entry.set_text("");
            }
        });
    }

    // Wire up browse button
    {
        let dialog_ref = dialog.clone();
        let key_path_entry = key_path_entry.clone();
        browse_button.connect_clicked(move |_| {
            show_file_chooser(&dialog_ref, &key_path_entry);
        });
    }

    // Port forwarding section - visible by default
    let forward_group = adw::PreferencesGroup::builder()
        .title("Port Forwarding")
        .description("Configure local port forwarding")
        .build();

    let local_host_entry = Entry::builder()
        .placeholder_text("127.0.0.1")
        .text("127.0.0.1")
        .build();
    let local_host_row = adw::ActionRow::builder()
        .title("Local Host")
        .subtitle("Bind address (127.0.0.1, 0.0.0.0, or specific IP)")
        .build();
    local_host_row.add_suffix(&local_host_entry);

    let local_port_spin = SpinButton::with_range(0.0, 65535.0, 1.0);
    local_port_spin.set_value(8080.0);
    let local_port_row = adw::ActionRow::builder()
        .title("Local Port")
        .subtitle("Port on your machine (0 = disabled)")
        .build();
    local_port_row.add_suffix(&local_port_spin);

    let remote_host_entry = Entry::builder()
        .placeholder_text("localhost")
        .text("localhost")
        .build();
    let remote_host_row = adw::ActionRow::builder()
        .title("Remote Host")
        .subtitle("Host to forward to (on remote server)")
        .build();
    remote_host_row.add_suffix(&remote_host_entry);

    let remote_port_spin = SpinButton::with_range(0.0, 65535.0, 1.0);
    remote_port_spin.set_value(80.0);
    let remote_port_row = adw::ActionRow::builder()
        .title("Remote Port")
        .subtitle("Port on remote host")
        .build();
    remote_port_row.add_suffix(&remote_port_spin);

    forward_group.add(&local_host_row);
    forward_group.add(&local_port_row);
    forward_group.add(&remote_host_row);
    forward_group.add(&remote_port_row);
    content_box.append(&forward_group);

    // Advanced tunnel options group with expander (collapsed by default)
    let advanced_group = adw::PreferencesGroup::builder()
        .title("Advanced Tunnel Options")
        .build();

    // Create expander row for advanced tuning settings
    let expander_row = adw::ExpanderRow::builder()
        .title("Advanced Tuning")
        .subtitle("Performance and reliability options")
        .build();

    // Get defaults for TunnelOptions
    let default_opts = TunnelOptions::default();

    // Compression
    let compression_switch = gtk4::Switch::new();
    compression_switch.set_active(default_opts.compression);
    compression_switch.set_valign(gtk4::Align::Center);
    let compression_row = adw::ActionRow::builder()
        .title("Compression")
        .subtitle("Enable SSH compression")
        .activatable(true)
        .build();
    compression_row.add_suffix(&compression_switch);
    compression_row.set_activatable_widget(Some(&compression_switch));

    // Keepalive interval
    let keepalive_spin = SpinButton::with_range(0.0, 300.0, 1.0);
    keepalive_spin.set_value(default_opts.keepalive_interval as f64);
    let keepalive_row = adw::ActionRow::builder()
        .title("Keepalive Interval")
        .subtitle("Seconds between keepalive packets (0 = disabled)")
        .build();
    keepalive_row.add_suffix(&keepalive_spin);

    // Auto-reconnect
    let auto_reconnect_switch = gtk4::Switch::new();
    auto_reconnect_switch.set_active(default_opts.auto_reconnect);
    auto_reconnect_switch.set_valign(gtk4::Align::Center);
    let auto_reconnect_row = adw::ActionRow::builder()
        .title("Auto-Reconnect")
        .subtitle("Automatically reconnect on connection loss")
        .activatable(true)
        .build();
    auto_reconnect_row.add_suffix(&auto_reconnect_switch);
    auto_reconnect_row.set_activatable_widget(Some(&auto_reconnect_switch));

    // Reconnect attempts
    let reconnect_attempts_spin = SpinButton::with_range(0.0, 100.0, 1.0);
    reconnect_attempts_spin.set_value(default_opts.reconnect_attempts as f64);
    let reconnect_attempts_row = adw::ActionRow::builder()
        .title("Reconnect Attempts")
        .subtitle("Maximum reconnect attempts (0 = unlimited)")
        .build();
    reconnect_attempts_row.add_suffix(&reconnect_attempts_spin);

    // Reconnect delay
    let reconnect_delay_spin = SpinButton::with_range(1.0, 60.0, 1.0);
    reconnect_delay_spin.set_value(default_opts.reconnect_delay as f64);
    let reconnect_delay_row = adw::ActionRow::builder()
        .title("Reconnect Delay")
        .subtitle("Seconds to wait before reconnecting")
        .build();
    reconnect_delay_row.add_suffix(&reconnect_delay_spin);

    // TCP Keepalive
    let tcp_keepalive_switch = gtk4::Switch::new();
    tcp_keepalive_switch.set_active(default_opts.tcp_keepalive);
    tcp_keepalive_switch.set_valign(gtk4::Align::Center);
    let tcp_keepalive_row = adw::ActionRow::builder()
        .title("TCP Keepalive")
        .subtitle("Enable TCP-level keepalive")
        .activatable(true)
        .build();
    tcp_keepalive_row.add_suffix(&tcp_keepalive_switch);
    tcp_keepalive_row.set_activatable_widget(Some(&tcp_keepalive_switch));

    // Max packet size
    let max_packet_spin = SpinButton::with_range(1024.0, 65536.0, 1024.0);
    max_packet_spin.set_value(default_opts.max_packet_size as f64);
    let max_packet_row = adw::ActionRow::builder()
        .title("Max Packet Size")
        .subtitle("Maximum SSH packet size in bytes")
        .build();
    max_packet_row.add_suffix(&max_packet_spin);

    // Window size
    let window_size_spin = SpinButton::with_range(32768.0, 2097152.0, 32768.0);
    window_size_spin.set_value(default_opts.window_size as f64);
    let window_size_row = adw::ActionRow::builder()
        .title("Window Size")
        .subtitle("SSH channel window size in bytes")
        .build();
    window_size_row.add_suffix(&window_size_spin);

    // Add all advanced option rows to expander
    expander_row.add_row(&compression_row);
    expander_row.add_row(&keepalive_row);
    expander_row.add_row(&auto_reconnect_row);
    expander_row.add_row(&reconnect_attempts_row);
    expander_row.add_row(&reconnect_delay_row);
    expander_row.add_row(&tcp_keepalive_row);
    expander_row.add_row(&max_packet_row);
    expander_row.add_row(&window_size_row);

    // Add expander to advanced group and advanced group to content
    advanced_group.add(&expander_row);
    content_box.append(&advanced_group);

    // Populate fields if editing existing profile
    if let Some(profile) = profile {
        if let Some(inner_profile) = profile.profile() {
            name_entry.set_text(&profile.name());
            host_entry.set_text(&profile.host());
            port_spin.set_value(profile.port() as f64);
            user_entry.set_text(&profile.user());

            // Populate auth fields
            if inner_profile.connection.auth_type == AuthType::Key {
                key_switch.set_active(true);
                if let Some(ref key_path) = inner_profile.connection.key_path {
                    key_path_entry.set_text(&key_path.to_string_lossy());
                }

                // Check if password is stored in keychain
                if inner_profile.connection.password_storage == PasswordStorage::Keychain {
                    // Enable keychain switch and show password entry
                    store_keychain_switch.set_active(true);
                    store_keychain_switch.set_sensitive(true);
                    key_password_row.set_visible(true);
                    key_password_row.set_subtitle("Passphrase stored in system keychain - enter new passphrase to update");
                }
            }

            // Populate forwarding fields
            local_host_entry.set_text(&inner_profile.forwarding.bind_address);
            local_port_spin.set_value(profile.local_port() as f64);
            remote_host_entry.set_text(&profile.remote_host());
            remote_port_spin.set_value(profile.remote_port() as f64);

            // Populate advanced tunnel options
            compression_switch.set_active(inner_profile.options.compression);
            keepalive_spin.set_value(inner_profile.options.keepalive_interval as f64);
            auto_reconnect_switch.set_active(inner_profile.options.auto_reconnect);
            reconnect_attempts_spin.set_value(inner_profile.options.reconnect_attempts as f64);
            reconnect_delay_spin.set_value(inner_profile.options.reconnect_delay as f64);
            tcp_keepalive_switch.set_active(inner_profile.options.tcp_keepalive);
            max_packet_spin.set_value(inner_profile.options.max_packet_size as f64);
            window_size_spin.set_value(inner_profile.options.window_size as f64);
        }
    }

    scrolled.set_child(Some(&content_box));
    toolbar_view.set_content(Some(&scrolled));

    // Wire up buttons
    {
        let dialog = dialog.clone();
        cancel_button.connect_clicked(move |_| {
            dialog.close();
        });
    }

    {
        let dialog = dialog.clone();
        let profile_id = profile.and_then(|p| p.profile()).map(|p| p.metadata.id);
        let state = state.clone();

        save_button.connect_clicked(move |_| {
            // Collect form data
            let name = name_entry.text().to_string();
            let host = host_entry.text().to_string();
            let port = port_spin.value() as u16;
            let user = user_entry.text().to_string();

            // Auth fields
            let use_key = key_switch.is_active();
            let key_path_text = key_path_entry.text().to_string();
            let key_password = key_password_entry.text().to_string();
            let store_in_keychain = store_keychain_switch.is_active();

            // Forwarding fields
            let local_host = local_host_entry.text().to_string();
            let local_port = local_port_spin.value() as u16;
            let remote_host = remote_host_entry.text().to_string();
            let remote_port = remote_port_spin.value() as u16;

            // Advanced tunnel options
            let compression = compression_switch.is_active();
            let keepalive_interval = keepalive_spin.value() as u64;
            let auto_reconnect = auto_reconnect_switch.is_active();
            let reconnect_attempts = reconnect_attempts_spin.value() as u32;
            let reconnect_delay = reconnect_delay_spin.value() as u64;
            let tcp_keepalive = tcp_keepalive_switch.is_active();
            let max_packet_size = max_packet_spin.value() as u32;
            let window_size = window_size_spin.value() as u32;

            // Validate required fields
            if name.trim().is_empty() {
                show_error_dialog(&dialog, "Profile name is required");
                return;
            }
            if host.trim().is_empty() {
                show_error_dialog(&dialog, "SSH host is required");
                return;
            }
            if user.trim().is_empty() {
                show_error_dialog(&dialog, "SSH user is required");
                return;
            }

            // Validate SSH key if enabled
            if use_key {
                if key_path_text.trim().is_empty() {
                    show_error_dialog(&dialog, "SSH key path is required when using key authentication");
                    return;
                }

                // Expand tilde in path
                let expanded_path = shellexpand::tilde(&key_path_text).to_string();
                let key_path = std::path::PathBuf::from(expanded_path);

                // Validate key file exists and has proper permissions
                if let Err(e) = validate_ssh_key(&key_path) {
                    show_error_dialog(&dialog, &e);
                    return;
                }

                // If storing in keychain, validate the passphrase works
                if store_in_keychain && !key_password.is_empty() {
                    if let Err(e) = validate_key_passphrase(&key_path, &key_password) {
                        show_error_dialog(
                            &dialog,
                            &format!("Cannot store passphrase: {}\n\nPlease verify the passphrase is correct.", e)
                        );
                        return;
                    }
                }
            }

            // Validate local host
            if local_host.trim().is_empty() {
                show_error_dialog(&dialog, "Local host/bind address is required");
                return;
            }

            // Check for duplicate profile name when creating new profile
            let is_new_profile = profile_id.is_none();
            if is_new_profile && ssh_tunnel_common::profile_exists_by_name(&name) {
                show_error_dialog(&dialog, &format!("A profile with the name '{}' already exists. Please choose a different name.", name));
                return;
            }

            // Create or update profile
            let now = chrono::Utc::now();
            let profile = Profile {
                metadata: ProfileMetadata {
                    id: profile_id.unwrap_or_else(|| Uuid::new_v4()),
                    name,
                    description: None,
                    created_at: if profile_id.is_some() {
                        // Keep original created_at if editing
                        // TODO: Pass original created_at from profile
                        now
                    } else {
                        now
                    },
                    modified_at: now,
                    tags: Vec::new(),
                },
                connection: ConnectionConfig {
                    host,
                    port,
                    user,
                    auth_type: if use_key { AuthType::Key } else { AuthType::Password },
                    key_path: if use_key && !key_path_text.trim().is_empty() {
                        let expanded_path = shellexpand::tilde(&key_path_text.trim()).to_string();
                        Some(std::path::PathBuf::from(expanded_path))
                    } else {
                        None
                    },
                    password_storage: if store_in_keychain && !key_password.is_empty() {
                        PasswordStorage::Keychain
                    } else {
                        PasswordStorage::None
                    },
                },
                forwarding: ForwardingConfig {
                    forwarding_type: ForwardingType::Local,
                    local_port: if local_port > 0 { Some(local_port) } else { None },
                    remote_host: if !remote_host.trim().is_empty() {
                        Some(remote_host)
                    } else {
                        None
                    },
                    remote_port: if remote_port > 0 { Some(remote_port) } else { None },
                    bind_address: local_host,
                },
                options: TunnelOptions {
                    compression,
                    keepalive_interval,
                    auto_reconnect,
                    reconnect_attempts,
                    reconnect_delay,
                    tcp_keepalive,
                    max_packet_size,
                    window_size,
                },
            };

            // Save profile using shared function from common crate
            // For new profiles: overwrite=false (will error if duplicate)
            // For editing: overwrite=true (allow updating existing profile)
            let overwrite = !is_new_profile;
            match ssh_tunnel_common::save_profile(&profile, overwrite) {
                Ok(path) => {
                    eprintln!("✓ Profile saved successfully: {}", path.display());
                }
                Err(e) => {
                    show_error_dialog(&dialog, &format!("Failed to save profile: {}", e));
                    return;
                }
            }

            // Handle keychain storage for SSH key passphrase
            let profile_id = profile.metadata.id;
            if store_in_keychain && !key_password.is_empty() {
                // Store passphrase in keychain
                if let Err(e) = store_password_in_keychain(&profile_id, &key_password) {
                    eprintln!("⚠️  Failed to store passphrase in keychain: {}", e);
                    show_error_dialog(&dialog, &format!("Profile saved, but failed to store passphrase in keychain: {}", e));
                    return;
                }
                eprintln!("✓ Passphrase stored in system keychain");
            } else if !store_in_keychain {
                // Remove from keychain if checkbox is unchecked
                if let Err(e) = remove_password_from_keychain(&profile_id) {
                    eprintln!("⚠️  Failed to remove passphrase from keychain: {}", e);
                    // Non-fatal error, continue
                }
            }

            // Close dialog
            dialog.close();

            // Reload the main profiles list page
            if let Some(list_box) = state.profile_list.borrow().as_ref() {
                super::profiles_list::populate_profiles(list_box, state.clone());
            }

            // If editing an existing profile, navigate back to the list
            if !is_new_profile {
                if let Some(nav_view) = state.nav_view.borrow().as_ref() {
                    nav_view.pop();
                }
            }
        });
    }

    dialog.set_content(Some(&toolbar_view));
    dialog
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

/// Show file chooser dialog for selecting SSH key
fn show_file_chooser(parent: &adw::Window, entry: &Entry) {
    use gtk4::gio;

    let file_chooser = gtk4::FileDialog::builder()
        .title("Select SSH Private Key")
        .modal(true)
        .build();

    // Set initial directory to ~/.ssh
    if let Some(home_dir) = dirs::home_dir() {
        let ssh_dir = home_dir.join(".ssh");
        if ssh_dir.exists() {
            let initial_folder = gio::File::for_path(&ssh_dir);
            file_chooser.set_initial_folder(Some(&initial_folder));
        }
    }

    // Add file filter for common key types
    let filters = gio::ListStore::new::<gtk4::FileFilter>();

    let filter = gtk4::FileFilter::new();
    filter.set_name(Some("SSH Keys"));
    filter.add_pattern("id_*");
    filter.add_pattern("*.pem");
    filter.add_pattern("*.key");
    filters.append(&filter);

    // Add "All files" filter
    let filter_all = gtk4::FileFilter::new();
    filter_all.set_name(Some("All Files"));
    filter_all.add_pattern("*");
    filters.append(&filter_all);

    file_chooser.set_filters(Some(&filters));

    let entry = entry.clone();
    let parent = parent.clone();
    file_chooser.open(Some(&parent), gio::Cancellable::NONE, move |result| {
        if let Ok(file) = result {
            if let Some(path) = file.path() {
                entry.set_text(&path.to_string_lossy());
            }
        }
    });
}

/// Validate SSH key file exists and has proper permissions
fn validate_ssh_key(key_path: &std::path::Path) -> Result<(), String> {
    use std::fs;

    // Check if file exists
    if !key_path.exists() {
        return Err(format!(
            "SSH key not found: {}\n\nCreate a key with: ssh-keygen -t ed25519",
            key_path.display()
        ));
    }

    // Check if it's a file (not a directory)
    if !key_path.is_file() {
        return Err(format!(
            "SSH key path is not a file: {}",
            key_path.display()
        ));
    }

    // Check permissions on Unix systems
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(key_path) {
            let permissions = metadata.permissions();
            let mode = permissions.mode();

            // SSH keys should be 0600 or 0400 (no group/other access)
            if mode & 0o077 != 0 {
                return Err(format!(
                    "SSH key has insecure permissions: {:o}\n\nFix with: chmod 600 {}",
                    mode & 0o777,
                    key_path.display()
                ));
            }
        }
    }

    Ok(())
}

/// Validate that the passphrase works with the SSH key
fn validate_key_passphrase(key_path: &std::path::Path, passphrase: &str) -> Result<(), String> {
    use russh_keys::decode_secret_key;
    use std::fs;

    // Read the key file
    let key_data = fs::read_to_string(key_path)
        .map_err(|e| format!("Failed to read SSH key file: {}", e))?;

    // Attempt to decode with the passphrase
    decode_secret_key(&key_data, Some(passphrase))
        .map_err(|e| format!("Invalid passphrase or corrupted key: {}", e))?;

    Ok(())
}

/// Store password/passphrase in system keychain
fn store_password_in_keychain(profile_id: &Uuid, password: &str) -> Result<(), String> {
    ssh_tunnel_common::store_password(profile_id, password)
        .map_err(|e| format!("{}", e))
}

/// Remove password/passphrase from system keychain
fn remove_password_from_keychain(profile_id: &Uuid) -> Result<(), String> {
    ssh_tunnel_common::remove_password(profile_id)
        .map_err(|e| format!("{}", e))
}
