// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Daemon status page - displays daemon information and control buttons

use gtk4::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;

use super::window::AppState;
use ssh_tunnel_common::{ConnectionMode, DaemonClientConfig, DaemonInfo};

/// Create the daemon status view
pub fn create(state: Rc<AppState>) -> adw::NavigationPage {
    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // Create header bar
    let header = adw::HeaderBar::new();
    header.set_show_back_button(false);
    header.add_css_class("flat"); // Match content background
    content_box.append(&header);

    // Create scrolled window with preferences page
    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);
    scrolled.set_hscrollbar_policy(gtk4::PolicyType::Never);

    // Create clamp for centered content
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(800);
    clamp.set_tightening_threshold(600);
    clamp.set_child(Some(&scrolled));

    content_box.append(&clamp);

    // Create navigation page
    let page = adw::NavigationPage::builder()
        .title("Daemon")
        .child(&content_box)
        .build();

    // Store references to scrolled window for later updates
    let scrolled_clone = scrolled.clone();
    let state_clone = state.clone();

    // Initial population
    populate_daemon_page(&scrolled, state.clone());

    // Set up refresh callback in AppState
    {
        let refresh_callback = Box::new(move || {
            populate_daemon_page(&scrolled_clone, state_clone.clone());
        });
        state.daemon_page_refresh.replace(Some(refresh_callback));
    }

    page
}

/// Populate or refresh the daemon page content
fn populate_daemon_page(scrolled: &gtk4::ScrolledWindow, state: Rc<AppState>) {
    // Load client configuration
    let config = load_client_config();

    // Check if we have a daemon client
    if state.daemon_client.borrow().is_none() {
        // No daemon client - show disconnected state
        let prefs_page = adw::PreferencesPage::new();
        add_connection_group(&prefs_page, &config, false);
        add_connection_error_banner(&prefs_page, "Daemon client not configured");
        scrolled.set_child(Some(&prefs_page));
        return;
    }

    // Clone what we need for the async task
    let scrolled = scrolled.clone();
    let state = state.clone();

    // Fetch daemon info asynchronously using glib's async runtime
    glib::MainContext::default().spawn_local(async move {
        let daemon_info_result = if let Some(client) = state.daemon_client.borrow().as_ref() {
            client.get_daemon_info().await
        } else {
            Err(anyhow::anyhow!("No daemon client"))
        };

        // Create new preferences page
        let prefs_page = adw::PreferencesPage::new();

        match daemon_info_result {
            Ok(daemon_info) => {
                // Successfully fetched daemon info - show all groups
                add_connection_group(&prefs_page, &config, true);
                add_daemon_config_group(&prefs_page, &daemon_info);
                add_file_locations_group(&prefs_page, &daemon_info);
                add_activity_group(&prefs_page, &daemon_info);
                add_actions_group(&prefs_page, state.clone(), &daemon_info);
            }
            Err(e) => {
                // Error fetching daemon info - daemon likely not connected
                eprintln!("Failed to fetch daemon info: {}", e);
                add_connection_group(&prefs_page, &config, false);
                add_connection_error_banner(&prefs_page, "Daemon is not connected");
            }
        }

        scrolled.set_child(Some(&prefs_page));
    });
}

/// Add connection status group
fn add_connection_group(
    prefs_page: &adw::PreferencesPage,
    config: &DaemonClientConfig,
    is_connected: bool,
) {
    let group = adw::PreferencesGroup::builder()
        .title("Connection")
        .build();

    // Connection status row
    let status_text = if is_connected { "Connected" } else { "Disconnected" };
    let status_row = adw::ActionRow::builder()
        .title("Status")
        .subtitle(status_text)
        .build();

    let status_icon = if is_connected {
        gtk4::Image::from_icon_name("emblem-ok-symbolic")
    } else {
        gtk4::Image::from_icon_name("dialog-error-symbolic")
    };
    status_icon.set_icon_size(gtk4::IconSize::Normal);
    status_row.add_suffix(&status_icon);

    group.add(&status_row);

    // Daemon type row
    let daemon_type = get_daemon_type(config);
    let type_row = adw::ActionRow::builder()
        .title("Type")
        .subtitle(&daemon_type)
        .build();
    group.add(&type_row);

    prefs_page.add(&group);
}

/// Add daemon configuration group
fn add_daemon_config_group(prefs_page: &adw::PreferencesPage, daemon_info: &DaemonInfo) {
    let group = adw::PreferencesGroup::builder()
        .title("Daemon Configuration")
        .build();

    // Version
    let version_row = adw::ActionRow::builder()
        .title("Version")
        .subtitle(&daemon_info.version)
        .build();
    group.add(&version_row);

    // Listener mode
    let listener_mode_display = match daemon_info.listener_mode.as_str() {
        "unix-socket" => "Unix Socket",
        "tcp-http" => "TCP (HTTP)",
        "tcp-https" => "TCP (HTTPS)",
        _ => &daemon_info.listener_mode,
    };
    let listener_row = adw::ActionRow::builder()
        .title("Listener Mode")
        .subtitle(listener_mode_display)
        .build();
    group.add(&listener_row);

    // Bind address or socket path
    if let Some(ref bind_host) = daemon_info.bind_host {
        if let Some(bind_port) = daemon_info.bind_port {
            let bind_addr = format!("{}:{}", bind_host, bind_port);
            let bind_row = adw::ActionRow::builder()
                .title("Bind Address")
                .subtitle(&bind_addr)
                .build();
            group.add(&bind_row);
        }
    } else if let Some(ref socket_path) = daemon_info.socket_path {
        let socket_row = adw::ActionRow::builder()
            .title("Socket Path")
            .subtitle(socket_path.as_str())
            .build();
        let copy_btn = create_copy_button(socket_path, "Copy socket path");
        socket_row.add_suffix(&copy_btn);
        group.add(&socket_row);
    }

    // Authentication
    let auth_text = if daemon_info.require_auth {
        "Required"
    } else {
        "Disabled"
    };
    let auth_row = adw::ActionRow::builder()
        .title("Authentication")
        .subtitle(auth_text)
        .build();
    group.add(&auth_row);

    // User
    let user_row = adw::ActionRow::builder()
        .title("User")
        .subtitle(&daemon_info.user)
        .build();
    group.add(&user_row);

    // PID
    let pid_row = adw::ActionRow::builder()
        .title("Process ID")
        .subtitle(&daemon_info.pid.to_string())
        .build();
    group.add(&pid_row);

    // Uptime
    let uptime_display = format_uptime(daemon_info.uptime_seconds);
    let uptime_row = adw::ActionRow::builder()
        .title("Uptime")
        .subtitle(&uptime_display)
        .build();
    group.add(&uptime_row);

    prefs_page.add(&group);
}

/// Add file locations group
fn add_file_locations_group(prefs_page: &adw::PreferencesPage, daemon_info: &DaemonInfo) {
    let group = adw::PreferencesGroup::builder()
        .title("File Locations")
        .build();

    // Config file
    let config_row = adw::ActionRow::builder()
        .title("Config File")
        .subtitle(&daemon_info.config_file_path)
        .build();
    let copy_btn = create_copy_button(&daemon_info.config_file_path, "Copy config file path");
    config_row.add_suffix(&copy_btn);
    group.add(&config_row);

    // Known hosts
    let hosts_row = adw::ActionRow::builder()
        .title("Known Hosts")
        .subtitle(&daemon_info.known_hosts_path)
        .build();
    let copy_btn = create_copy_button(&daemon_info.known_hosts_path, "Copy known hosts path");
    hosts_row.add_suffix(&copy_btn);
    group.add(&hosts_row);

    prefs_page.add(&group);
}

/// Add activity group
fn add_activity_group(prefs_page: &adw::PreferencesPage, daemon_info: &DaemonInfo) {
    let group = adw::PreferencesGroup::builder()
        .title("Activity")
        .build();

    // Active tunnels count
    let count_text = if daemon_info.active_tunnels_count == 1 {
        "1 tunnel".to_string()
    } else {
        format!("{} tunnels", daemon_info.active_tunnels_count)
    };
    let tunnels_row = adw::ActionRow::builder()
        .title("Active Tunnels")
        .subtitle(&count_text)
        .build();

    // Add info icon with tooltip
    let info_icon = gtk4::Image::from_icon_name("help-about-symbolic");
    info_icon.set_tooltip_text(Some("Includes tunnels from all users"));
    tunnels_row.add_suffix(&info_icon);

    group.add(&tunnels_row);

    // Started at
    let started_row = adw::ActionRow::builder()
        .title("Started At")
        .subtitle(&daemon_info.started_at)
        .build();
    group.add(&started_row);

    prefs_page.add(&group);
}

/// Add actions group (stop button and restart info)
fn add_actions_group(prefs_page: &adw::PreferencesPage, state: Rc<AppState>, daemon_info: &DaemonInfo) {
    let group = adw::PreferencesGroup::builder()
        .title("Actions")
        .build();

    // Stop button row
    let stop_row = adw::ActionRow::builder()
        .title("Stop Daemon")
        .subtitle("Shutdown the daemon (all tunnels will be disconnected)")
        .build();

    let stop_btn = gtk4::Button::builder()
        .label("Stop")
        .valign(gtk4::Align::Center)
        .css_classes(vec!["destructive-action".to_string()])
        .build();

    // Wire up stop button
    {
        let state = state.clone();
        stop_btn.connect_clicked(move |btn| {
            show_stop_confirmation_dialog(btn, state.clone());
        });
    }

    stop_row.add_suffix(&stop_btn);
    group.add(&stop_row);

    // Only show restart info for unix-socket mode
    // For HTTPS remote daemons, restart is misleading (daemon runs on remote host)
    if daemon_info.listener_mode == "unix-socket" {
        let restart_row = adw::ActionRow::builder()
            .title("Restart Daemon")
            .subtitle("Use 'systemctl --user restart ssh-tunnel-daemon' or manually restart")
            .build();

        let info_icon = gtk4::Image::from_icon_name("help-about-symbolic");
        restart_row.add_suffix(&info_icon);

        group.add(&restart_row);
    }

    prefs_page.add(&group);
}

/// Add connection error banner
fn add_connection_error_banner(prefs_page: &adw::PreferencesPage, message: &str) {
    let group = adw::PreferencesGroup::new();

    let banner = adw::Banner::new(message);
    banner.set_revealed(true);

    // Wrap banner in a box to add it to the preferences page
    let banner_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    banner_box.append(&banner);

    // We can't directly add a banner to a PreferencesGroup, so wrap it in an ActionRow
    let row = adw::ActionRow::new();
    row.set_child(Some(&banner_box));

    group.add(&row);
    prefs_page.add(&group);
}

/// Show stop confirmation dialog
fn show_stop_confirmation_dialog(widget: &gtk4::Button, state: Rc<AppState>) {
    let window = widget.root().and_then(|root| root.downcast::<gtk4::Window>().ok());

    if let Some(window) = window {
        let dialog = adw::MessageDialog::builder()
            .transient_for(&window)
            .heading("Stop the Daemon?")
            .body("All active tunnels will be disconnected. The daemon will shutdown.\n\nIf running as a systemd service, it may automatically restart.")
            .build();

        dialog.add_response("cancel", "Cancel");
        dialog.add_response("stop", "Stop Daemon");
        dialog.set_response_appearance("stop", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        let state = state.clone();
        dialog.connect_response(None, move |_, response| {
            if response == "stop" {
                // Spawn async task to shutdown daemon
                let state = state.clone();
                glib::MainContext::default().spawn_local(async move {
                    if let Some(client) = state.daemon_client.borrow().as_ref() {
                        match client.shutdown_daemon().await {
                            Ok(()) => {
                                eprintln!("Daemon shutdown initiated");

                                // Immediately refresh the daemon page to show disconnected state
                                if let Some(refresh) = state.daemon_page_refresh.borrow().as_ref() {
                                    eprintln!("✓ Refreshing daemon page (daemon stopped via UI)");
                                    refresh();
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to shutdown daemon: {}", e);
                            }
                        }
                    }
                });
            }
        });

        dialog.present();
    }
}

/// Determine daemon type from client config
fn get_daemon_type(config: &DaemonClientConfig) -> String {
    match config.connection_mode {
        ConnectionMode::UnixSocket => {
            if let Ok(path) = config.socket_path() {
                let path_str = path.display().to_string();
                if path_str.contains("/run/user/") {
                    "Local User Daemon".to_string()
                } else {
                    "Local System Daemon".to_string()
                }
            } else {
                "Local Daemon".to_string()
            }
        }
        ConnectionMode::Http | ConnectionMode::Https => {
            if config.daemon_host == "localhost"
                || config.daemon_host == "127.0.0.1"
                || config.daemon_host == "::1"
            {
                format!("Local Network Daemon ({})", config.daemon_host)
            } else {
                format!("Remote Network Daemon ({})", config.daemon_host)
            }
        }
    }
}

/// Format uptime in human-readable format
fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;

    if days > 0 {
        format!("{} days {} hours", days, hours)
    } else if hours > 0 {
        format!("{} hours {} minutes", hours, minutes)
    } else if minutes > 0 {
        format!("{} minutes", minutes)
    } else {
        format!("{} seconds", seconds)
    }
}

/// Create copy button for paths
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

/// Load client configuration
fn load_client_config() -> DaemonClientConfig {
    // Try to load from CLI config file location
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

