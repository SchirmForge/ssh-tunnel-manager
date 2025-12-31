// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! First-launch configuration wizard

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation};
use libadwaita as adw;
use adw::prelude::*;
use ssh_tunnel_common::{is_valid_host, ConnectionMode, DaemonClientConfig};
use ssh_tunnel_gui_core::{check_config_status, load_snippet_config, ConfigStatus};

/// Show configuration wizard and return configured DaemonClientConfig
/// Returns None if user cancels
pub fn show_config_wizard(parent: Option<&impl IsA<gtk4::Window>>) -> Option<DaemonClientConfig> {
    let status = check_config_status();

    match status {
        ConfigStatus::Exists => {
            // Configuration already exists, no wizard needed
            None
        }
        ConfigStatus::SnippetAvailable => {
            // Show snippet import dialog
            match show_snippet_import_dialog(parent) {
                Some(config) => Some(config),
                None => {
                    // User declined snippet, show manual config dialog
                    show_manual_config_dialog(parent)
                }
            }
        }
        ConfigStatus::NeedsSetup => {
            // Show manual configuration dialog
            show_manual_config_dialog(parent)
        }
    }
}

/// Show dialog to import configuration from daemon-generated snippet
fn show_snippet_import_dialog(parent: Option<&impl IsA<gtk4::Window>>) -> Option<DaemonClientConfig> {
    use std::cell::Cell;
    use std::rc::Rc;

    let dialog = adw::MessageDialog::new(
        parent,
        Some("Configuration Detected"),
        Some("A daemon configuration file was found. Would you like to import it?"),
    );

    dialog.add_response("no", "No");
    dialog.add_response("yes", "Yes");
    dialog.set_response_appearance("yes", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("yes"));

    // Use a cell to store the result
    let result = Rc::new(Cell::new(None));
    let result_clone = result.clone();

    dialog.connect_response(None, move |_dialog, response| {
        if response == "yes" {
            match load_snippet_config() {
                Ok(mut config) => {
                    // Check if daemon_host is empty for HTTP/HTTPS modes
                    if matches!(config.connection_mode, ConnectionMode::Http | ConnectionMode::Https)
                        && config.daemon_host.is_empty() {
                        // Prompt for IP address
                        if let Some(ip) = prompt_for_ip_address(_dialog.transient_for().as_ref()) {
                            config.daemon_host = ip;
                            result_clone.set(Some(config));
                        }
                        // If user cancels IP prompt, result stays None
                    } else {
                        result_clone.set(Some(config));
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load snippet config: {}", e);
                }
            }
        }
    });

    dialog.set_modal(true);
    dialog.present();

    // Run nested event loop to make this modal
    let loop_ref = glib::MainLoop::new(None, false);
    let loop_clone = loop_ref.clone();
    dialog.connect_close_request(move |_| {
        loop_clone.quit();
        glib::Propagation::Proceed
    });

    loop_ref.run();

    result.take()
}

/// Prompt user for daemon IP address
fn prompt_for_ip_address(parent: Option<&impl IsA<gtk4::Window>>) -> Option<String> {
    use std::cell::Cell;
    use std::rc::Rc;

    let dialog = adw::MessageDialog::new(
        parent,
        Some("Daemon IP Address Required"),
        Some("The daemon is configured to listen on all network interfaces.\nPlease specify the IP address to connect to:"),
    );

    // Create entry field for IP address
    let entry = adw::EntryRow::new();
    entry.set_title("Daemon IP Address");
    entry.set_text("192.168.1.100");  // Suggested default

    let prefs_group = adw::PreferencesGroup::new();
    prefs_group.add(&entry);

    // Add to dialog (using extra_child if available, or message area)
    dialog.set_extra_child(Some(&prefs_group));

    dialog.add_response("cancel", "Cancel");
    dialog.add_response("ok", "OK");
    dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("ok"));

    // Handle Enter key in entry field to submit
    // EntryRow doesn't have connect_activate directly, need to use connect_entry_activated
    let dialog_activate = dialog.clone();
    entry.connect_entry_activated(move |_| {
        dialog_activate.response("ok");
    });

    let result = Rc::new(Cell::new(None));
    let result_clone = result.clone();
    let entry_clone = entry.clone();

    dialog.connect_response(None, move |_dialog, response| {
        if response == "ok" {
            let ip = entry_clone.text().to_string().trim().to_string();

            // Validate IP address or hostname
            if !ip.is_empty() && is_valid_host(&ip) {
                result_clone.set(Some(ip));
            } else if !ip.is_empty() {
                // Show error for invalid IP
                if let Some(window) = _dialog.transient_for() {
                    let error_dialog = adw::MessageDialog::new(
                        Some(&window),
                        Some("Invalid IP Address"),
                        Some("Please enter a valid IP address (IPv4 or IPv6) or hostname.\n\nExamples:\n• 192.168.1.100\n• 10.0.0.5\n• 2001:db8::1\n• daemon.local"),
                    );
                    error_dialog.add_response("ok", "OK");
                    error_dialog.set_default_response(Some("ok"));
                    error_dialog.present();
                }
            }
        }
    });

    dialog.set_modal(true);
    dialog.present();

    // Run nested event loop
    let loop_ref = glib::MainLoop::new(None, false);
    let loop_clone = loop_ref.clone();
    dialog.connect_close_request(move |_| {
        loop_clone.quit();
        glib::Propagation::Proceed
    });

    loop_ref.run();

    result.take()
}

/// Show manual configuration dialog
fn show_manual_config_dialog(parent: Option<&impl IsA<gtk4::Window>>) -> Option<DaemonClientConfig> {
    let dialog = adw::PreferencesWindow::new();
    dialog.set_title(Some("Configure Daemon Connection"));
    dialog.set_modal(true);
    dialog.set_default_size(500, 550);
    dialog.set_search_enabled(false);

    if let Some(parent_window) = parent {
        dialog.set_transient_for(Some(parent_window.as_ref()));
    }

    // Create preferences page
    let prefs_page = adw::PreferencesPage::new();

    // Connection mode selector
    let mode_group = adw::PreferencesGroup::new();
    mode_group.set_title("Connection Mode");
    mode_group.set_description(Some("Choose how to connect to the SSH Tunnel Manager daemon"));

    let socket_row = adw::ActionRow::new();
    socket_row.set_title("Unix Socket (Local)");
    socket_row.set_subtitle("Connect via local socket (recommended for same-machine)");
    let socket_check = gtk4::CheckButton::new();
    socket_check.set_active(true);
    socket_row.add_suffix(&socket_check);
    socket_row.set_activatable_widget(Some(&socket_check));
    mode_group.add(&socket_row);

    let https_row = adw::ActionRow::new();
    https_row.set_title("HTTPS (Network)");
    https_row.set_subtitle("Connect via HTTPS (for remote daemon or different user)");
    let https_check = gtk4::CheckButton::new();
    https_check.set_group(Some(&socket_check));
    https_row.add_suffix(&https_check);
    https_row.set_activatable_widget(Some(&https_check));
    mode_group.add(&https_row);

    prefs_page.add(&mode_group);

    // HTTPS settings (hidden by default)
    let https_settings = adw::PreferencesGroup::new();
    https_settings.set_title("HTTPS Settings");
    https_settings.set_description(Some("Required when connecting to a remote daemon"));
    https_settings.set_visible(false);

    let host_row = adw::EntryRow::new();
    host_row.set_title("Daemon Host");
    host_row.set_text("127.0.0.1");
    https_settings.add(&host_row);

    let port_row = adw::EntryRow::new();
    port_row.set_title("Daemon Port");
    port_row.set_text("3443");
    https_settings.add(&port_row);

    let fingerprint_row = adw::EntryRow::new();
    fingerprint_row.set_title("TLS Certificate Fingerprint");
    fingerprint_row.set_show_apply_button(false);
    https_settings.add(&fingerprint_row);

    prefs_page.add(&https_settings);

    // Authentication token (always shown)
    let auth_group = adw::PreferencesGroup::new();
    auth_group.set_title("Authentication");
    auth_group.set_description(Some("Required - daemon enforces authentication for all connection modes"));

    let token_row = adw::PasswordEntryRow::new();
    token_row.set_title("Authentication Token");
    auth_group.add(&token_row);

    prefs_page.add(&auth_group);

    // Add the preferences page to the window
    dialog.add(&prefs_page);

    // Actions buttons - add them to the window directly
    let button_box = GtkBox::new(Orientation::Horizontal, 12);
    button_box.set_halign(gtk4::Align::Center);
    button_box.set_margin_top(12);
    button_box.set_margin_bottom(24);
    button_box.set_margin_start(12);
    button_box.set_margin_end(12);

    let cancel_button = Button::with_label("Cancel");
    cancel_button.set_size_request(120, -1);
    button_box.append(&cancel_button);

    let use_config_button = Button::with_label("Use This Configuration");
    use_config_button.add_css_class("suggested-action");
    use_config_button.set_size_request(200, -1);
    button_box.append(&use_config_button);

    // Create an action row for buttons (no styling, just a container)
    let button_row = adw::ActionRow::new();
    button_row.set_activatable(false);
    button_row.set_child(Some(&button_box));

    let button_group = adw::PreferencesGroup::new();
    button_group.add(&button_row);
    prefs_page.add(&button_group);

    // Use a cell to store the result
    use std::cell::Cell;
    use std::rc::Rc;
    let result = Rc::new(Cell::new(None));

    // Toggle HTTPS settings visibility
    let https_settings_clone = https_settings.clone();
    https_check.connect_toggled(move |check| {
        https_settings_clone.set_visible(check.is_active());
    });

    // Cancel button
    let result_cancel = result.clone();
    let dialog_clone1 = dialog.clone();
    cancel_button.connect_clicked(move |_| {
        result_cancel.set(None);
        dialog_clone1.close();
    });

    // Use Configuration button
    let result_save = result.clone();
    let dialog_clone2 = dialog.clone();
    let socket_check_clone = socket_check.clone();
    let host_row_clone = host_row.clone();
    let port_row_clone = port_row.clone();
    let fingerprint_row_clone = fingerprint_row.clone();
    let token_row_clone = token_row.clone();

    use_config_button.connect_clicked(move |button| {
        // Auth token is required for all modes
        let auth_token = token_row_clone.text().to_string();

        if auth_token.is_empty() {
            // Show error dialog
            if let Some(window) = button.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
                let dialog = adw::MessageDialog::new(
                    Some(&window),
                    Some("Authentication Token Required"),
                    Some("The daemon requires an authentication token for all connection modes. Please provide the token from the daemon."),
                );
                dialog.add_response("ok", "OK");
                dialog.set_default_response(Some("ok"));
                dialog.present();
            }
            return;
        }

        let config = if socket_check_clone.is_active() {
            // Unix socket mode
            DaemonClientConfig {
                connection_mode: ConnectionMode::UnixSocket,
                daemon_host: "127.0.0.1".to_string(),
                daemon_port: 3443,
                daemon_url: String::new(),
                auth_token,
                tls_cert_fingerprint: String::new(),
                skip_ssh_setup_warning: false,
            }
        } else {
            // HTTPS mode - also requires fingerprint
            let fingerprint = fingerprint_row_clone.text().to_string();

            if fingerprint.is_empty() {
                // Show error dialog
                if let Some(window) = button.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
                    let dialog = adw::MessageDialog::new(
                        Some(&window),
                        Some("TLS Certificate Fingerprint Required"),
                        Some("For HTTPS mode, the TLS Certificate Fingerprint is required for secure connection verification."),
                    );
                    dialog.add_response("ok", "OK");
                    dialog.set_default_response(Some("ok"));
                    dialog.present();
                }
                return;
            }

            let port = port_row_clone.text().parse::<u16>().unwrap_or(3443);
            DaemonClientConfig {
                connection_mode: ConnectionMode::Https,
                daemon_host: host_row_clone.text().to_string(),
                daemon_port: port,
                daemon_url: String::new(),
                auth_token,
                tls_cert_fingerprint: fingerprint,
                skip_ssh_setup_warning: false,
            }
        };

        result_save.set(Some(config));
        dialog_clone2.close();
    });

    dialog.set_modal(true);
    dialog.present();

    // Run nested event loop to make this modal
    let loop_ref = glib::MainLoop::new(None, false);
    let loop_clone = loop_ref.clone();
    dialog.connect_close_request(move |_| {
        loop_clone.quit();
        glib::Propagation::Proceed
    });

    loop_ref.run();

    result.take()
}
