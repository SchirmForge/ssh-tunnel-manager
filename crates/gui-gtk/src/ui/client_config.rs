// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Client Configuration page - displays daemon connection settings (read-only)

use gtk4::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;

use super::window::AppState;
use ssh_tunnel_common::{ConnectionMode, DaemonClientConfig};

/// Create the Client Configuration page (read-only view of daemon client config)
pub fn create(state: Rc<AppState>) -> adw::NavigationPage {
    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // Header bar (back button provided automatically by NavigationView)
    let header_bar = adw::HeaderBar::new();
    main_box.append(&header_bar);

    // Create preferences page with groups
    let prefs_page = adw::PreferencesPage::new();

    // Check if there's a pending configuration
    if let Some(pending_config) = state.pending_daemon_config.borrow().as_ref() {
        // Show pending configuration with save button
        add_pending_config_group(&prefs_page, &state, pending_config);
    } else {
        // Load and show saved configuration
        let config = load_client_config();

        // 1. Connection Information Group
        add_connection_group(&prefs_page, &config);

        // 2. Authentication Group
        add_auth_group(&prefs_page, &config);

        // 3. File Locations Group
        add_file_locations_group(&prefs_page, &state);
    }

    // 4. Connection Status Group (always shown)
    add_status_group(&prefs_page, &state);

    // Wrap in clamp for centered content
    let clamp = adw::Clamp::builder()
        .maximum_size(800)
        .tightening_threshold(600)
        .child(&prefs_page)
        .build();

    // Wrap in scrolled window
    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .child(&clamp)
        .build();

    main_box.append(&scrolled);

    // Create navigation page
    let page = adw::NavigationPage::builder()
        .title("Client Configuration")
        .child(&main_box)
        .build();

    page
}

/// Add connection information group
fn add_connection_group(prefs_page: &adw::PreferencesPage, config: &DaemonClientConfig) {
    let group = adw::PreferencesGroup::builder()
        .title("Connection")
        .description("How the GUI connects to the daemon")
        .build();

    // Connection mode
    let mode_text = match config.connection_mode {
        ConnectionMode::UnixSocket => "Unix Socket (local)",
        ConnectionMode::Http => "HTTP (localhost)",
        ConnectionMode::Https => "HTTPS (network)",
    };

    let mode_row = adw::ActionRow::builder()
        .title("Connection Mode")
        .subtitle(mode_text)
        .build();
    group.add(&mode_row);

    // Daemon endpoint
    let endpoint = match config.connection_mode {
        ConnectionMode::UnixSocket => {
            config.daemon_base_url()
                .unwrap_or_else(|_| "Unix socket".to_string())
        }
        ConnectionMode::Http | ConnectionMode::Https => {
            format!("{}:{}", config.daemon_host, config.daemon_port)
        }
    };

    let endpoint_row = adw::ActionRow::builder()
        .title("Daemon Endpoint")
        .subtitle(&endpoint)
        .build();
    group.add(&endpoint_row);

    prefs_page.add(&group);
}

/// Add authentication group
fn add_auth_group(prefs_page: &adw::PreferencesPage, config: &DaemonClientConfig) {
    let group = adw::PreferencesGroup::builder()
        .title("Authentication")
        .build();

    // Auth token (masked)
    let token_display = if config.auth_token.is_empty() {
        "Not configured".to_string()
    } else {
        let len = config.auth_token.len();
        if len > 8 {
            format!("{}...{}", &config.auth_token[..4], &config.auth_token[len-4..])
        } else {
            "****".to_string()
        }
    };

    let token_row = adw::ActionRow::builder()
        .title("Authentication Token")
        .subtitle(&token_display)
        .build();

    // Add copy button if token exists
    if !config.auth_token.is_empty() {
        let copy_btn = create_copy_button(&config.auth_token, "Copy token");
        token_row.add_suffix(&copy_btn);
    }

    group.add(&token_row);

    // TLS fingerprint (if HTTPS mode)
    if matches!(config.connection_mode, ConnectionMode::Https) {
        let fingerprint_display = if config.tls_cert_fingerprint.is_empty() {
            "Not configured".to_string()
        } else {
            config.tls_cert_fingerprint.clone()
        };

        let fp_row = adw::ActionRow::builder()
            .title("TLS Certificate Fingerprint")
            .subtitle(&fingerprint_display)
            .build();

        // Add copy button if fingerprint exists
        if !config.tls_cert_fingerprint.is_empty() {
            let copy_btn = create_copy_button(&config.tls_cert_fingerprint, "Copy fingerprint");
            fp_row.add_suffix(&copy_btn);
        }

        group.add(&fp_row);
    }

    prefs_page.add(&group);
}

/// Add pending configuration group with save button
fn add_pending_config_group(prefs_page: &adw::PreferencesPage, state: &Rc<AppState>, config: &DaemonClientConfig) {
    let group = adw::PreferencesGroup::builder()
        .title("Pending Configuration")
        .description("This configuration has not been saved yet. Review and save to apply.")
        .build();

    // Connection mode
    let mode_text = match config.connection_mode {
        ConnectionMode::UnixSocket => "Unix Socket (local)",
        ConnectionMode::Http => "HTTP (localhost)",
        ConnectionMode::Https => "HTTPS (network)",
    };

    let mode_row = adw::ActionRow::builder()
        .title("Connection Mode")
        .subtitle(mode_text)
        .build();
    group.add(&mode_row);

    // Daemon endpoint
    let endpoint = match config.connection_mode {
        ConnectionMode::UnixSocket => {
            config.daemon_base_url()
                .unwrap_or_else(|_| "Unix socket".to_string())
        }
        ConnectionMode::Http | ConnectionMode::Https => {
            format!("{}:{}", config.daemon_host, config.daemon_port)
        }
    };

    let endpoint_row = adw::ActionRow::builder()
        .title("Daemon Endpoint")
        .subtitle(&endpoint)
        .build();
    group.add(&endpoint_row);

    // Auth token status
    let token_status = if config.auth_token.is_empty() {
        "Not configured"
    } else {
        "Configured"
    };

    let token_row = adw::ActionRow::builder()
        .title("Authentication Token")
        .subtitle(token_status)
        .build();
    group.add(&token_row);

    // TLS fingerprint (if HTTPS mode)
    if matches!(config.connection_mode, ConnectionMode::Https) {
        let fingerprint_status = if config.tls_cert_fingerprint.is_empty() {
            "Not configured"
        } else {
            "Configured"
        };

        let fp_row = adw::ActionRow::builder()
            .title("TLS Certificate Fingerprint")
            .subtitle(fingerprint_status)
            .build();
        group.add(&fp_row);
    }

    // Save button row
    let button_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    button_box.set_halign(gtk4::Align::Center);
    button_box.set_margin_top(12);
    button_box.set_margin_bottom(12);

    let save_button = gtk4::Button::with_label("Save Configuration");
    save_button.add_css_class("suggested-action");
    save_button.set_size_request(200, -1);
    button_box.append(&save_button);

    let button_row = adw::ActionRow::builder()
        .activatable(false)
        .build();
    button_row.set_child(Some(&button_box));
    group.add(&button_row);

    // Connect save button handler
    let state_clone = state.clone();
    save_button.connect_clicked(move |button| {
        // Clone the pending config to avoid borrow conflicts
        let pending_config = state_clone.pending_daemon_config.borrow().clone();

        if let Some(config) = pending_config {
            match ssh_tunnel_gui_core::save_daemon_config(&config) {
                Ok(()) => {
                    // Clear pending config and modified flag
                    state_clone.pending_daemon_config.replace(None);
                    state_clone.config_modified.replace(false);

                    // Update daemon client with saved configuration
                    match ssh_tunnel_gui_core::DaemonClient::with_config(config) {
                        Ok(client) => {
                            state_clone.daemon_client.replace(Some(client));
                            eprintln!("Daemon client updated with saved configuration");
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to create daemon client with saved config: {}", e);
                        }
                    }

                    // Show success message
                    if let Some(window) = button.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
                        let dialog = adw::MessageDialog::new(
                            Some(&window),
                            Some("Configuration Saved"),
                            Some("The daemon configuration has been saved successfully."),
                        );
                        dialog.add_response("ok", "OK");
                        dialog.set_default_response(Some("ok"));
                        dialog.present();
                    }

                    // TODO: Refresh the page to show saved config
                    // This would require rebuilding the NavigationPage
                }
                Err(e) => {
                    // Show error dialog
                    if let Some(window) = button.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
                        let dialog = adw::MessageDialog::new(
                            Some(&window),
                            Some("Save Failed"),
                            Some(&format!("Failed to save configuration: {}", e)),
                        );
                        dialog.add_response("ok", "OK");
                        dialog.set_default_response(Some("ok"));
                        dialog.present();
                    }
                }
            }
        }
    });

    prefs_page.add(&group);
}

/// Add file locations group
fn add_file_locations_group(prefs_page: &adw::PreferencesPage, state: &Rc<AppState>) {
    let group = adw::PreferencesGroup::builder()
        .title("Configuration Files")
        .build();

    // CLI config file path
    let config_path = get_cli_config_path();
    let config_path_row = adw::ActionRow::builder()
        .title("CLI Config File")
        .subtitle(&config_path)
        .build();
    group.add(&config_path_row);

    // Config snippet path with import button if available
    let snippet_info = get_snippet_info();
    let snippet_row = adw::ActionRow::builder()
        .title("Daemon Config Snippet")
        .subtitle(&snippet_info)
        .build();

    // Add Import button if snippet is available
    if ssh_tunnel_gui_core::daemon_config_snippet_exists() {
        let import_button = gtk4::Button::with_label("Import Snippet");
        import_button.add_css_class("flat");
        import_button.set_valign(gtk4::Align::Center);

        let state_clone = state.clone();
        import_button.connect_clicked(move |button| {
            // Load snippet config
            match ssh_tunnel_gui_core::load_snippet_config() {
                Ok(config) => {
                    // Set as pending config
                    state_clone.pending_daemon_config.replace(Some(config));
                    state_clone.config_modified.replace(true);

                    // Show success message
                    if let Some(window) = button.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
                        let dialog = adw::MessageDialog::new(
                            Some(&window),
                            Some("Snippet Imported"),
                            Some("The daemon configuration snippet has been imported. Review it above and click Save to apply."),
                        );
                        dialog.add_response("ok", "OK");
                        dialog.set_default_response(Some("ok"));
                        dialog.present();
                    }

                    // TODO: Refresh the page to show pending config
                    // This would require rebuilding the NavigationPage
                }
                Err(e) => {
                    // Show error dialog
                    if let Some(window) = button.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
                        let dialog = adw::MessageDialog::new(
                            Some(&window),
                            Some("Import Failed"),
                            Some(&format!("Failed to import configuration snippet: {}", e)),
                        );
                        dialog.add_response("ok", "OK");
                        dialog.set_default_response(Some("ok"));
                        dialog.present();
                    }
                }
            }
        });

        snippet_row.add_suffix(&import_button);
    }

    group.add(&snippet_row);

    prefs_page.add(&group);
}

/// Add connection status group
fn add_status_group(prefs_page: &adw::PreferencesPage, state: &AppState) {
    let group = adw::PreferencesGroup::builder()
        .title("Connection Status")
        .build();

    // Daemon reachability
    let is_connected = state.daemon_client.borrow().is_some();
    let status_text = if is_connected {
        "Connected"
    } else {
        "Disconnected"
    };

    let status_row = adw::ActionRow::builder()
        .title("Daemon Status")
        .subtitle(status_text)
        .build();

    // Add status indicator icon
    let status_icon = if is_connected {
        gtk4::Image::from_icon_name("emblem-ok-symbolic")
    } else {
        gtk4::Image::from_icon_name("dialog-error-symbolic")
    };
    status_icon.set_icon_size(gtk4::IconSize::Normal);
    status_row.add_suffix(&status_icon);

    group.add(&status_row);

    prefs_page.add(&group);
}

/// Create a copy button for text
fn create_copy_button(text: &str, tooltip: &str) -> gtk4::Button {
    let button = gtk4::Button::builder()
        .icon_name("edit-copy-symbolic")
        .valign(gtk4::Align::Center)
        .tooltip_text(tooltip)
        .css_classes(vec!["flat".to_string()])
        .build();

    let text_clone = text.to_string();
    button.connect_clicked(move |_| {
        if let Some(display) = gtk4::gdk::Display::default() {
            display.clipboard().set_text(&text_clone);
        }
    });

    button
}

/// Load client configuration (GUI uses same config as CLI)
fn load_client_config() -> DaemonClientConfig {
    // Try to load from CLI config file location
    // This reuses the same configuration structure as the CLI
    match load_cli_config_file() {
        Ok(config) => config,
        Err(_) => DaemonClientConfig::default(),
    }
}

/// Load CLI config file (reuses CLI's config structure)
fn load_cli_config_file() -> Result<DaemonClientConfig, Box<dyn std::error::Error>> {
    use std::fs;

    let config_path = dirs::config_dir()
        .ok_or("Could not determine config directory")?
        .join("ssh-tunnel-manager")
        .join("cli.toml");

    if !config_path.exists() {
        return Ok(DaemonClientConfig::default());
    }

    let contents = fs::read_to_string(&config_path)?;

    // Parse the TOML - the CLI config wraps DaemonClientConfig
    #[derive(serde::Deserialize)]
    struct CliConfig {
        #[serde(flatten)]
        daemon_config: DaemonClientConfig,
    }

    let cli_config: CliConfig = toml::from_str(&contents)?;
    Ok(cli_config.daemon_config)
}

/// Get CLI config file path as display string
fn get_cli_config_path() -> String {
    dirs::config_dir()
        .map(|dir| {
            dir.join("ssh-tunnel-manager")
                .join("cli.toml")
                .display()
                .to_string()
        })
        .unwrap_or_else(|| "Unknown".to_string())
}

/// Get daemon config snippet info (path + availability status)
fn get_snippet_info() -> String {
    match ssh_tunnel_common::get_cli_config_snippet_path() {
        Ok(path) => {
            if path.exists() {
                format!("{} (available)", path.display())
            } else {
                format!("{} (not found)", path.display())
            }
        }
        Err(_) => "Unknown".to_string(),
    }
}
