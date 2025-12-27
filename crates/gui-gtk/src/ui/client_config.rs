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

    // Load current configuration
    let config = load_client_config();

    // Create preferences page with groups
    let prefs_page = adw::PreferencesPage::new();

    // 1. Connection Information Group
    add_connection_group(&prefs_page, &config);

    // 2. Authentication Group
    add_auth_group(&prefs_page, &config);

    // 3. File Locations Group
    add_file_locations_group(&prefs_page);

    // 4. Connection Status Group
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

/// Add file locations group
fn add_file_locations_group(prefs_page: &adw::PreferencesPage) {
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

    // Config snippet path
    let snippet_info = get_snippet_info();
    let snippet_row = adw::ActionRow::builder()
        .title("Daemon Config Snippet")
        .subtitle(&snippet_info)
        .build();
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
