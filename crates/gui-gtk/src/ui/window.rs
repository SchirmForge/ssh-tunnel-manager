// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Main application window

use gtk4::{prelude::*, gio};
use libadwaita as adw;
use adw::prelude::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::cell::RefCell;

use super::{navigation, profiles_list, daemon_settings, help_dialog, about_dialog};
use super::auth_dialog;
use crate::models::profile_model::ProfileModel;
use crate::daemon::DaemonClient;
use ssh_tunnel_common::AuthRequest;

/// Shared application state
pub struct AppState {
    pub selected_profile: RefCell<Option<ProfileModel>>,
    pub details_widget: RefCell<Option<gtk4::Box>>,
    pub window: RefCell<Option<adw::ApplicationWindow>>,
    pub profile_list: RefCell<Option<gtk4::ListBox>>,
    pub daemon_client: RefCell<Option<DaemonClient>>,
    pub current_nav_page: RefCell<navigation::NavigationPage>,
    pub nav_view: RefCell<Option<adw::NavigationView>>,
    // Direct references to navigation pages (cleaner than title-based lookup)
    pub client_page: RefCell<Option<adw::NavigationPage>>,
    pub daemon_page: RefCell<Option<adw::NavigationPage>>,
    // Profile details page widgets for real-time updates
    pub profile_details_banner: RefCell<Option<adw::Banner>>,
    pub profile_details_start_btn: RefCell<Option<gtk4::Button>>,
    pub profile_details_stop_btn: RefCell<Option<gtk4::Button>>,
    pub auth_dialog_open: RefCell<HashSet<uuid::Uuid>>,
    pub pending_auth_requests: RefCell<HashMap<uuid::Uuid, AuthRequest>>,
    pub active_auth_requests: RefCell<HashMap<uuid::Uuid, AuthRequest>>,
    // Callback to refresh daemon page content
    pub daemon_page_refresh: RefCell<Option<Box<dyn Fn()>>>,
}

impl AppState {
    fn new() -> Rc<Self> {
        // Load CLI configuration to connect to daemon
        let daemon_client = match Self::load_daemon_client() {
            Ok(client) => Some(client),
            Err(e) => {
                eprintln!("Warning: Failed to create daemon client: {}", e);
                None
            }
        };

        Rc::new(Self {
            selected_profile: RefCell::new(None),
            details_widget: RefCell::new(None),
            window: RefCell::new(None),
            profile_list: RefCell::new(None),
            daemon_client: RefCell::new(daemon_client),
            current_nav_page: RefCell::new(navigation::NavigationPage::Client),
            nav_view: RefCell::new(None),
            client_page: RefCell::new(None),
            daemon_page: RefCell::new(None),
            daemon_page_refresh: RefCell::new(None),
            profile_details_banner: RefCell::new(None),
            profile_details_start_btn: RefCell::new(None),
            profile_details_stop_btn: RefCell::new(None),
            auth_dialog_open: RefCell::new(HashSet::new()),
            pending_auth_requests: RefCell::new(HashMap::new()),
            active_auth_requests: RefCell::new(HashMap::new()),
        })
    }

    /// Load daemon client configuration from cli.toml
    fn load_daemon_client() -> anyhow::Result<DaemonClient> {
        use ssh_tunnel_common::DaemonClientConfig;
        use std::path::PathBuf;

        // Get config path (same as CLI)
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        let config_path: PathBuf = config_dir.join("ssh-tunnel-manager").join("cli.toml");

        eprintln!("Loading daemon config from: {:?}", config_path);

        // Load configuration from file
        let config = if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)
                .map_err(|e| anyhow::anyhow!("Failed to read config file: {}", e))?;
            let cfg = toml::from_str::<DaemonClientConfig>(&contents)
                .map_err(|e| anyhow::anyhow!("Failed to parse config file: {}", e))?;

            eprintln!("Loaded config: connection_mode={:?}, daemon_url={}",
                     cfg.connection_mode, cfg.daemon_url);
            cfg
        } else {
            eprintln!("Config file not found at {:?}, using defaults", config_path);
            DaemonClientConfig::default()
        };

        // Create client with loaded config
        DaemonClient::with_config(config)
    }
}

/// Build the main application window
pub fn build(app: &adw::Application) -> adw::ApplicationWindow {
    // Create shared state
    let state = AppState::new();

    // Create main window (no window controls, like GNOME Settings)
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("SSH Tunnel Manager")
        .default_width(1000)
        .default_height(700)
        .build();

    // Create daemon connection status indicator
    // Start in "connecting" state - the event listener will update it
    let status_icon = gtk4::Image::from_icon_name("emblem-synchronizing-symbolic");
    status_icon.set_tooltip_text(Some("Connecting to daemon..."));
    status_icon.add_css_class("daemon-connecting");

    // Create navigation split view for sidebar + content
    let split_view = adw::NavigationSplitView::new();
    split_view.set_min_sidebar_width(250.0);
    split_view.set_max_sidebar_width(350.0);

    // Store window reference in state
    state.window.replace(Some(window.clone()));

    // Register Help action
    {
        let window_clone = window.clone();
        let help_action = gio::SimpleAction::new("help", None);
        help_action.connect_activate(move |_, _| {
            help_dialog::show_help_dialog(&window_clone);
        });
        app.add_action(&help_action);
    }

    // Register About action
    {
        let window_clone = window.clone();
        let about_action = gio::SimpleAction::new("about", None);
        about_action.connect_activate(move |_, _| {
            about_dialog::show_about_dialog(&window_clone);
        });
        app.add_action(&about_action);
    }

    // Start event listener for real-time updates
    start_event_listener(state.clone(), status_icon.clone());

    // Create navigation sidebar (left panel with Profiles and Daemon options)
    let nav_sidebar = navigation::create(state.clone(), status_icon);

    let nav_sidebar_page = adw::NavigationPage::builder()
        .title("Navigation")
        .child(&nav_sidebar)
        .build();

    // Create right panel with NavigationView for page switching
    let nav_view = adw::NavigationView::new();

    // Store nav_view reference in state so navigation can switch pages
    state.nav_view.replace(Some(nav_view.clone()));

    // Create Client page (default)
    let client_page = profiles_list::create(state.clone());
    nav_view.add(&client_page);
    state.client_page.replace(Some(client_page));

    // Create daemon settings page
    let daemon_page = daemon_settings::create(state.clone());
    nav_view.add(&daemon_page);
    state.daemon_page.replace(Some(daemon_page));

    // Wrap navigation view in a page
    let content_page = adw::NavigationPage::builder()
        .title("Content")
        .child(&nav_view)
        .build();

    // Set up split view
    split_view.set_sidebar(Some(&nav_sidebar_page));
    split_view.set_content(Some(&content_page));
    split_view.set_show_content(true);

    // Set split view as window content (no toolbar wrapper)
    window.set_content(Some(&split_view));

    window
}

/// Start listening to daemon events for real-time updates
fn start_event_listener(state: Rc<AppState>, status_icon: gtk4::Image) {
    use crate::daemon::sse::EventListener;
    use crate::daemon::sse::TunnelEvent;

    // Get daemon config
    let config = match AppState::load_daemon_client() {
        Ok(client) => client.config.clone(),
        Err(_) => return, // Can't connect, skip event listener
    };

    // Spawn event listener task with reconnection logic
    glib::MainContext::default().spawn_local(async move {
        let mut backoff = tokio::time::Duration::from_secs(2);
        let max_backoff = tokio::time::Duration::from_secs(30);

        // Keep reconnecting indefinitely
        loop {
            // Indicate we're trying to connect
            status_icon.set_icon_name(Some("emblem-synchronizing-symbolic"));
            status_icon.set_tooltip_text(Some("Connecting to daemon..."));
            status_icon.remove_css_class("daemon-connected");
            status_icon.remove_css_class("daemon-offline");
            status_icon.add_css_class("daemon-connecting");

            let listener = EventListener::new(config.clone());

            match listener.listen().await {
                Ok(mut rx) => {
                    // Wait for first event with timeout to verify connection
                    match tokio::time::timeout(tokio::time::Duration::from_secs(5), rx.recv()).await {
                        Ok(Some(_first_event)) => {
                            eprintln!("✓ Event listener connected (received first event)");
                            status_icon.set_icon_name(Some("network-wired-symbolic"));
                            status_icon.set_tooltip_text(Some("✓ Connected to daemon"));
                            status_icon.remove_css_class("daemon-offline");
                            status_icon.remove_css_class("daemon-connecting");
                            status_icon.add_css_class("daemon-connected");

                            // Reset backoff on successful connection
                            backoff = tokio::time::Duration::from_secs(2);

                            // Refresh daemon page if we're currently viewing it
                            if *state.current_nav_page.borrow() == navigation::NavigationPage::Daemon {
                                if let Some(refresh) = state.daemon_page_refresh.borrow().as_ref() {
                                    eprintln!("✓ Refreshing daemon page (daemon connected)");
                                    refresh();
                                }
                            }

                            // Query initial tunnel statuses for all profiles
                            if let Some(client) = state.daemon_client.borrow().as_ref() {
                                if let Some(list_box) = state.profile_list.borrow().as_ref() {
                                    let list_box_clone = list_box.clone();
                                    let client_clone = client.clone();
                                    let state_clone = state.clone();

                                    glib::MainContext::default().spawn_local(async move {
                                        match client_clone.list_tunnels().await {
                                            Ok(tunnels) => {
                                                eprintln!("✓ Queried {} tunnel statuses from daemon", tunnels.len());
                                                for tunnel in tunnels {
                                                    super::profiles_list::update_profile_status(
                                                        &list_box_clone,
                                                        tunnel.id,
                                                        tunnel.status,
                                                    );

                                                    if let Some(request) = tunnel.pending_auth {
                                                        if let Some(window) = state_clone.window.borrow().as_ref() {
                                                            super::auth_dialog::handle_auth_request(
                                                                window,
                                                                request,
                                                                state_clone.clone(),
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!("⚠ Failed to query tunnel statuses: {}", e);
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        Ok(None) => {
                            eprintln!("✗ Event channel closed before first event");
                            continue; // Retry connection
                        }
                        Err(_) => {
                            eprintln!("✗ Timeout waiting for first event from daemon");
                            continue; // Retry connection
                        }
                    }

                // Create a channel for heartbeat timeout signal
                let (timeout_tx, mut timeout_rx) = tokio::sync::mpsc::unbounded_channel();

                // Track last heartbeat time using Arc for sharing across tasks
                // Initialize to NOW so the monitor doesn't trigger immediately
                let last_heartbeat = std::sync::Arc::new(std::sync::Mutex::new(tokio::time::Instant::now()));
                let last_heartbeat_clone = last_heartbeat.clone();

                // Spawn heartbeat monitor task
                tokio::spawn(async move {
                    let heartbeat_timeout = tokio::time::Duration::from_secs(30);

                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                        let elapsed = {
                            let last = last_heartbeat_clone.lock().unwrap();
                            last.elapsed()
                        };

                        if elapsed > heartbeat_timeout {
                            // Timeout reached - send signal to exit event loop
                            eprintln!("✗ Heartbeat timeout - daemon appears offline");
                            let _ = timeout_tx.send(());
                            break;
                        }
                    }
                });

                // Process events as they arrive
                loop {
                    tokio::select! {
                        Some(event) = rx.recv() => {
                            eprintln!("Received event: {:?}", event);

                    // Update UI based on event type
                    match event {
                        TunnelEvent::Connected { id } => {
                            eprintln!("Tunnel {} connected", id);
                            auth_dialog::clear_auth_state(&state, id);

                            // Update profile list status
                            if let Some(list_box) = state.profile_list.borrow().as_ref() {
                                super::profiles_list::update_profile_status(
                                    list_box,
                                    id,
                                    ssh_tunnel_common::TunnelStatus::Connected,
                                );
                            }

                            // Update profile details page if this profile is selected
                            if let Some(selected) = state.selected_profile.borrow().as_ref() {
                                if let Some(profile) = selected.profile() {
                                    if profile.metadata.id == id {
                                        // Update profile details page UI
                                        super::profile_details::update_tunnel_status(
                                            &state,
                                            ssh_tunnel_common::TunnelStatus::Connected,
                                        );

                                        // Also refresh old details panel if present
                                        if let Some(details_widget) = state.details_widget.borrow().as_ref() {
                                            if let Some(window) = state.window.borrow().as_ref() {
                                                super::details::update_with_profile(
                                                    details_widget,
                                                    selected,
                                                    state.clone(),
                                                    window,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        TunnelEvent::Starting { id } => {
                            eprintln!("Tunnel {} starting", id);
                            auth_dialog::clear_auth_state(&state, id);

                            // Update profile list status
                            if let Some(list_box) = state.profile_list.borrow().as_ref() {
                                super::profiles_list::update_profile_status(
                                    list_box,
                                    id,
                                    ssh_tunnel_common::TunnelStatus::Connecting,
                                );
                            }

                            // Update profile details page if this profile is selected
                            if let Some(selected) = state.selected_profile.borrow().as_ref() {
                                if let Some(profile) = selected.profile() {
                                    if profile.metadata.id == id {
                                        super::profile_details::update_tunnel_status(
                                            &state,
                                            ssh_tunnel_common::TunnelStatus::Connecting,
                                        );
                                    }
                                }
                            }
                        }
                        TunnelEvent::Disconnected { id, reason } => {
                            eprintln!("Tunnel {} disconnected: {}", id, reason);
                            auth_dialog::clear_auth_state(&state, id);

                            // Update profile list status
                            if let Some(list_box) = state.profile_list.borrow().as_ref() {
                                super::profiles_list::update_profile_status(
                                    list_box,
                                    id,
                                    ssh_tunnel_common::TunnelStatus::Disconnected,
                                );
                            }

                            // Update profile details page if this profile is selected
                            if let Some(selected) = state.selected_profile.borrow().as_ref() {
                                if let Some(profile) = selected.profile() {
                                    if profile.metadata.id == id {
                                        super::profile_details::update_tunnel_status(
                                            &state,
                                            ssh_tunnel_common::TunnelStatus::Disconnected,
                                        );

                                        // Also refresh old details panel if present
                                        if let Some(details_widget) = state.details_widget.borrow().as_ref() {
                                            if let Some(window) = state.window.borrow().as_ref() {
                                                super::details::update_with_profile(
                                                    details_widget,
                                                    selected,
                                                    state.clone(),
                                                    window,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        TunnelEvent::Error { id, error } => {
                            eprintln!("Tunnel {} error: {}", id, error);
                            auth_dialog::clear_auth_state(&state, id);

                            // Update profile list status
                            if let Some(list_box) = state.profile_list.borrow().as_ref() {
                                super::profiles_list::update_profile_status(
                                    list_box,
                                    id,
                                    ssh_tunnel_common::TunnelStatus::Failed(error.clone()),
                                );
                            }

                            // Update profile details page if this profile is selected
                            if let Some(selected) = state.selected_profile.borrow().as_ref() {
                                if let Some(profile) = selected.profile() {
                                    if profile.metadata.id == id {
                                        super::profile_details::update_tunnel_status(
                                            &state,
                                            ssh_tunnel_common::TunnelStatus::Failed(error.clone()),
                                        );

                                        // Also refresh old details panel if present
                                        if let Some(details_widget) = state.details_widget.borrow().as_ref() {
                                            if let Some(window) = state.window.borrow().as_ref() {
                                                super::details::update_with_profile(
                                                    details_widget,
                                                    selected,
                                                    state.clone(),
                                                    window,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        TunnelEvent::AuthRequired { id, request } => {
                            eprintln!("Auth required for tunnel {}: {}", id, request.prompt);

                            // Update profile list status
                            if let Some(list_box) = state.profile_list.borrow().as_ref() {
                                super::profiles_list::update_profile_status(
                                    list_box,
                                    id,
                                    ssh_tunnel_common::TunnelStatus::WaitingForAuth,
                                );
                            }

                            // Update profile details page status
                            if let Some(selected) = state.selected_profile.borrow().as_ref() {
                                if let Some(profile) = selected.profile() {
                                    if profile.metadata.id == id {
                                        super::profile_details::update_tunnel_status(
                                            &state,
                                            ssh_tunnel_common::TunnelStatus::WaitingForAuth,
                                        );
                                    }
                                }
                            }

                            // Show auth dialog if window is available
                            if let Some(window) = state.window.borrow().as_ref() {
                                super::auth_dialog::handle_auth_request(
                                    window,
                                    request,
                                    state.clone(),
                                );
                            }

                            // Also refresh old details panel
                            if let Some(selected) = state.selected_profile.borrow().as_ref() {
                                if let Some(profile) = selected.profile() {
                                    if profile.metadata.id == id {
                                        if let Some(details_widget) = state.details_widget.borrow().as_ref() {
                                            if let Some(window) = state.window.borrow().as_ref() {
                                                super::details::update_with_profile(
                                                    details_widget,
                                                    selected,
                                                    state.clone(),
                                                    window,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        TunnelEvent::Heartbeat { .. } => {
                            // Update heartbeat timestamp
                            {
                                let mut last = last_heartbeat.lock().unwrap();
                                *last = tokio::time::Instant::now();
                            }

                            // Use heartbeat to reflect healthy connection
                            status_icon.set_icon_name(Some("network-wired-symbolic"));
                            status_icon.set_tooltip_text(Some("✓ Connected to daemon"));
                            status_icon.remove_css_class("daemon-offline");
                            status_icon.remove_css_class("daemon-connecting");
                            status_icon.add_css_class("daemon-connected");
                        }
                    }
                        }
                        _ = timeout_rx.recv() => {
                            // Heartbeat timeout - daemon appears offline
                            eprintln!("✗ Daemon heartbeat timeout");
                            status_icon.set_icon_name(Some("network-offline-symbolic"));
                            status_icon.set_tooltip_text(Some("✗ Connection timeout"));
                            status_icon.remove_css_class("daemon-connected");
                            status_icon.remove_css_class("daemon-connecting");
                            status_icon.add_css_class("daemon-offline");

                            // Refresh daemon page if we're currently viewing it
                            if *state.current_nav_page.borrow() == navigation::NavigationPage::Daemon {
                                if let Some(refresh) = state.daemon_page_refresh.borrow().as_ref() {
                                    eprintln!("✓ Refreshing daemon page (daemon disconnected)");
                                    refresh();
                                }
                            }

                            // Reset all profile statuses to NotConnected (gray)
                            if let Some(list_box) = state.profile_list.borrow().as_ref() {
                                let mut index = 0;
                                while let Some(row) = list_box.row_at_index(index) {
                                    if let Some(action_row) = row.downcast_ref::<adw::ActionRow>() {
                                        if let Some(stored_id) = unsafe { action_row.data::<String>("profile_id") } {
                                            let profile_id_str: &String = unsafe { stored_id.as_ref() };
                                            if let Ok(profile_id) = uuid::Uuid::parse_str(profile_id_str) {
                                                super::profiles_list::update_profile_status(
                                                    list_box,
                                                    profile_id,
                                                    ssh_tunnel_common::TunnelStatus::NotConnected,
                                                );
                                            }
                                        }
                                    }
                                    index += 1;
                                }
                            }

                            break; // Exit event loop and reconnect
                        }
                    }
                }
                    // Event loop exited (timeout) - will reconnect after backoff
                    eprintln!("Reconnecting to daemon in {} seconds...", backoff.as_secs());
                }
                Err(e) => {
                    eprintln!("✗ Failed to start event listener: {}", e);
                    status_icon.set_icon_name(Some("network-offline-symbolic"));
                    status_icon.set_tooltip_text(Some("✗ Failed to connect to daemon"));
                    status_icon.remove_css_class("daemon-connecting");
                    status_icon.remove_css_class("daemon-connected");
                    status_icon.add_css_class("daemon-offline");
                }
            }

            // Wait before retrying with exponential backoff
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(max_backoff);
        }
    });
}
