// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Main application window

use gtk4::{prelude::*, gio};
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::VecDeque;

use super::{navigation, profiles_list, daemon_settings, help_dialog, about_dialog};
use crate::models::profile_model::ProfileModel;
use crate::daemon::DaemonClient;
use ssh_tunnel_gui_core::AppCore;
use ssh_tunnel_common::{DaemonClientConfig, AuthRequest};

/// Shared application state
pub struct AppState {
    // Business logic (from gui-core) - framework-agnostic
    pub core: RefCell<AppCore>,

    // GTK-specific UI state below
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
    // Callback to refresh daemon page content
    pub daemon_page_refresh: RefCell<Option<Box<dyn Fn()>>>,
    // Pending daemon configuration (not yet saved to file)
    pub pending_daemon_config: RefCell<Option<DaemonClientConfig>>,
    // Track if configuration has been modified during this session
    pub config_modified: RefCell<bool>,
    // Active auth dialog (for closing on retry)
    pub active_auth_dialog: RefCell<Option<adw::MessageDialog>>,
    // Active auth request ID (for correlation)
    pub active_auth_request_id: RefCell<Option<uuid::Uuid>>,
    // Event queue for auth requests (decouples SSE reception from dialog display)
    pub auth_request_queue: RefCell<VecDeque<AuthRequest>>,
    // Flag indicating if we're currently processing an auth request
    pub processing_auth_request: RefCell<bool>,
}

impl AppState {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            // Initialize core business logic
            core: RefCell::new(AppCore::new()),
            // GTK-specific state
            selected_profile: RefCell::new(None),
            details_widget: RefCell::new(None),
            window: RefCell::new(None),
            profile_list: RefCell::new(None),
            daemon_client: RefCell::new(None), // Will be set after config wizard if needed
            current_nav_page: RefCell::new(navigation::NavigationPage::Client),
            nav_view: RefCell::new(None),
            client_page: RefCell::new(None),
            daemon_page: RefCell::new(None),
            daemon_page_refresh: RefCell::new(None),
            profile_details_banner: RefCell::new(None),
            profile_details_start_btn: RefCell::new(None),
            profile_details_stop_btn: RefCell::new(None),
            pending_daemon_config: RefCell::new(None),
            config_modified: RefCell::new(false),
            active_auth_dialog: RefCell::new(None),
            active_auth_request_id: RefCell::new(None),
            auth_request_queue: RefCell::new(VecDeque::new()),
            processing_auth_request: RefCell::new(false),
        })
    }

    /// Load daemon client configuration from cli.toml
    fn load_daemon_client() -> anyhow::Result<DaemonClient> {
        // Use gui-core's helper to load daemon config
        let config = ssh_tunnel_gui_core::load_daemon_config()?;

        eprintln!("Loaded daemon config: connection_mode={:?}", config.connection_mode);

        // Create client with loaded config
        DaemonClient::with_config(config)
    }

    /// Initialize daemon client with configuration (call after window is created)
    fn init_daemon_client(&self) {
        match Self::load_daemon_client() {
            Ok(client) => {
                self.daemon_client.replace(Some(client));
            }
            Err(e) => {
                eprintln!("Warning: Failed to create daemon client: {}", e);
            }
        }
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

    // Check for first-launch configuration and run wizard if needed
    use ssh_tunnel_gui_core::check_config_status;
    let config_status = check_config_status();

    if config_status != ssh_tunnel_gui_core::ConfigStatus::Exists {
        // Show configuration wizard
        if let Some(config) = super::config_wizard::show_config_wizard(Some(&window)) {
            eprintln!("Configuration provided by wizard");
            // Store pending configuration
            state.pending_daemon_config.replace(Some(config.clone()));
            state.config_modified.replace(true);

            // Create daemon client with this config (but don't save to file yet)
            match DaemonClient::with_config(config) {
                Ok(client) => {
                    state.daemon_client.replace(Some(client));
                }
                Err(e) => {
                    eprintln!("Warning: Failed to create daemon client: {}", e);
                }
            }
        } else {
            eprintln!("Configuration wizard canceled");
        }
    } else {
        // Configuration exists, load it
        state.init_daemon_client();
    }

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

    // Handle window close - prompt to save config if modified
    let state_close = state.clone();
    let window_clone = window.clone();
    window.connect_close_request(move |_| {
        if *state_close.config_modified.borrow() {
            // Config was modified, ask to save
            let dialog = adw::MessageDialog::new(
                Some(&window_clone),
                Some("Save Configuration?"),
                Some("Your daemon configuration has not been saved. Save it before closing?"),
            );

            dialog.add_response("discard", "Discard");
            dialog.add_response("save", "Save");
            dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("save"));

            let state_dialog = state_close.clone();
            let window_dialog = window_clone.clone();
            dialog.connect_response(None, move |_, response| {
                if response == "save" {
                    if let Some(config) = state_dialog.pending_daemon_config.borrow().clone() {
                        match ssh_tunnel_gui_core::save_daemon_config(&config) {
                            Ok(()) => {
                                eprintln!("Configuration saved successfully");
                                state_dialog.config_modified.replace(false);

                                // Update daemon client with saved configuration
                                match ssh_tunnel_gui_core::DaemonClient::with_config(config) {
                                    Ok(client) => {
                                        state_dialog.daemon_client.replace(Some(client));
                                        eprintln!("Daemon client updated with saved configuration");
                                    }
                                    Err(e) => {
                                        eprintln!("Warning: Failed to create daemon client: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to save configuration: {}", e);
                            }
                        }
                    }
                } else if response == "discard" {
                    // User chose to discard changes
                    state_dialog.config_modified.replace(false);
                }
                // Destroy the window directly to avoid triggering close-request again
                window_dialog.destroy();
            });

            dialog.present();
            glib::Propagation::Stop // Prevent immediate close
        } else {
            glib::Propagation::Proceed // Allow close
        }
    });

    window
}

/// Start listening to daemon events for real-time updates
fn start_event_listener(state: Rc<AppState>, status_icon: gtk4::Image) {
    use crate::daemon::{EventListener, TunnelEvent};

    // Get daemon config from state's daemon_client or pending config
    let config = {
        if let Some(client) = state.daemon_client.borrow().as_ref() {
            // Use existing daemon client config
            client.config.clone()
        } else if let Some(pending) = state.pending_daemon_config.borrow().as_ref() {
            // Use pending config from wizard
            pending.clone()
        } else {
            // Try loading from file as fallback
            match ssh_tunnel_gui_core::load_daemon_config() {
                Ok(config) => {
                    eprintln!("Loaded daemon config: connection_mode={:?}", config.connection_mode);
                    config
                }
                Err(_) => return, // Can't load config, skip event listener
            }
        }
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
                            tracing::info!("SSE event stream connected to daemon");
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
                                    tracing::debug!("Refreshing daemon page (daemon connected)");
                                    refresh();
                                }
                            }
                        }
                        Ok(None) => {
                            tracing::warn!("SSE event channel closed before first event - retrying");
                            continue; // Retry connection
                        }
                        Err(_) => {
                            tracing::warn!("Timeout waiting for first SSE event - retrying");
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
                            tracing::warn!("Heartbeat timeout - daemon appears offline");
                            let _ = timeout_tx.send(());
                            break;
                        }
                    }
                });

                // Sync tunnel state after SSE connection established
                // This catches any AuthRequired events that were emitted during disconnection
                tracing::info!("SSE connected - syncing tunnel state from daemon");
                if let Some(client) = state.daemon_client.borrow().as_ref() {
                    if let Some(list_box) = state.profile_list.borrow().as_ref() {
                        let list_box_clone = list_box.clone();
                        let client_clone = client.clone();
                        let state_clone = state.clone();

                        glib::MainContext::default().spawn_local(async move {
                            match client_clone.list_tunnels().await {
                                Ok(tunnels) => {
                                    tracing::info!("Synced {} tunnel statuses after SSE reconnect", tunnels.len());
                                    for tunnel in tunnels {
                                        // Update status in AppCore
                                        {
                                            let mut core = state_clone.core.borrow_mut();
                                            core.tunnel_statuses.insert(tunnel.id, tunnel.status.clone());
                                        }

                                        // Update UI
                                        super::profiles_list::update_profile_status(
                                            &list_box_clone,
                                            tunnel.id,
                                            tunnel.status.clone(),
                                        );

                                        // Handle any pending auth requests that were missed during disconnect
                                        if let Some(request) = tunnel.pending_auth {
                                            tracing::info!("Found pending auth for tunnel {} after reconnect - queueing", tunnel.id);
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
                                    tracing::warn!("Failed to sync tunnel statuses after reconnect: {}", e);
                                }
                            }
                        });
                    }
                }

                // Process events as they arrive
                loop {
                    tokio::select! {
                        Some(event) = rx.recv() => {
                            tracing::debug!("Received SSE event: {:?}", event);

                            // Use centralized event handler from gui-core
                            super::event_handler::process_tunnel_event(&state, event.clone());

                            // Update profile details page if a profile is selected
                            // (This is GTK-specific UI that's not in the centralized handler)
                            match event {
                                TunnelEvent::Connected { id } |
                                TunnelEvent::Starting { id } |
                                TunnelEvent::Disconnected { id, .. } |
                                TunnelEvent::Error { id, .. } => {
                                    if let Some(selected) = state.selected_profile.borrow().as_ref() {
                                        if let Some(profile) = selected.profile() {
                                            if profile.metadata.id == id {
                                                // Get the status from AppCore
                                                let status = {
                                                    let core = state.core.borrow();
                                                    core.tunnel_statuses.get(&id).cloned()
                                                        .unwrap_or(ssh_tunnel_common::TunnelStatus::NotConnected)
                                                };

                                                // Update profile details page UI
                                                super::profile_details::update_tunnel_status(&state, status.clone());

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
                                TunnelEvent::AuthRequired { id, .. } => {
                                    // Update profile details page if this is the selected profile
                                    if let Some(selected) = state.selected_profile.borrow().as_ref() {
                                        if let Some(profile) = selected.profile() {
                                            if profile.metadata.id == id {
                                                // Update profile details page UI
                                                super::profile_details::update_tunnel_status(
                                                    &state,
                                                    ssh_tunnel_common::TunnelStatus::WaitingForAuth,
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
                                TunnelEvent::Heartbeat { .. } => {
                                    // Update heartbeat timestamp
                                    {
                                        let mut last = last_heartbeat.lock().unwrap();
                                        *last = tokio::time::Instant::now();
                                    }

                                    // Update daemon connection state in AppCore
                                    super::event_handler::handle_daemon_connected(&state, true);

                                    // Use heartbeat to reflect healthy connection
                                    status_icon.set_icon_name(Some("network-wired-symbolic"));
                                    status_icon.set_tooltip_text(Some("✓ Connected to daemon"));
                                    status_icon.remove_css_class("daemon-offline");
                                    status_icon.remove_css_class("daemon-connecting");
                                    status_icon.add_css_class("daemon-connected");

                                    // Periodic status sync on heartbeat to catch missed SSE events
                                    // Only sync tunnels in transitional states (Connecting, WaitingForAuth)
                                    let tunnels_to_check: Vec<uuid::Uuid> = {
                                        let core = state.core.borrow();
                                        core.tunnel_statuses.iter()
                                            .filter(|(_, status)| status.is_in_progress())
                                            .map(|(id, _)| *id)
                                            .collect()
                                    };

                                    if !tunnels_to_check.is_empty() {
                                        tracing::debug!(
                                            "Heartbeat backup poll: Checking {} transitional tunnels (per-tunnel polling is primary)",
                                            tunnels_to_check.len()
                                        );

                                        if let Some(client) = state.daemon_client.borrow().as_ref() {
                                            if let Some(list_box) = state.profile_list.borrow().as_ref() {
                                                let client_clone = client.clone();
                                                let state_clone = state.clone();
                                                let list_box_clone = list_box.clone();

                                                glib::MainContext::default().spawn_local(async move {
                                                    for tunnel_id in tunnels_to_check {
                                                        match client_clone.get_tunnel_status(tunnel_id).await {
                                                            Ok(Some(response)) => {
                                                                // Update status in AppCore
                                                                {
                                                                    let mut core = state_clone.core.borrow_mut();
                                                                    let old_status = core.tunnel_statuses.get(&tunnel_id).cloned();

                                                                    // Only update if status changed
                                                                    if old_status.as_ref() != Some(&response.status) {
                                                                        tracing::info!(
                                                                            "Heartbeat backup poll: Tunnel {} status changed from {:?} to {:?}",
                                                                            tunnel_id, old_status, response.status
                                                                        );
                                                                        core.tunnel_statuses.insert(tunnel_id, response.status.clone());
                                                                    }
                                                                }

                                                                // Update UI
                                                                super::profiles_list::update_profile_status(
                                                                    &list_box_clone,
                                                                    tunnel_id,
                                                                    response.status.clone(),
                                                                );

                                                                // Handle missed auth requests
                                                                if let Some(request) = response.pending_auth {
                                                                    tracing::info!(
                                                                        "Heartbeat backup poll: Found pending auth for tunnel {} - queueing",
                                                                        tunnel_id
                                                                    );
                                                                    if let Some(window) = state_clone.window.borrow().as_ref() {
                                                                        super::auth_dialog::handle_auth_request(
                                                                            window,
                                                                            request,
                                                                            state_clone.clone(),
                                                                        );
                                                                    }
                                                                }

                                                                // Trigger status event handler for terminal states
                                                                match &response.status {
                                                                    ssh_tunnel_common::TunnelStatus::Connected |
                                                                    ssh_tunnel_common::TunnelStatus::Disconnected |
                                                                    ssh_tunnel_common::TunnelStatus::Failed(_) => {
                                                                        super::event_handler::handle_status_changed(
                                                                            &state_clone,
                                                                            tunnel_id,
                                                                            response.status.clone(),
                                                                        );
                                                                    }
                                                                    _ => {}
                                                                }
                                                            }
                                                            Ok(None) => {
                                                                tracing::debug!(
                                                                    "Heartbeat backup poll: Tunnel {} not found (404)",
                                                                    tunnel_id
                                                                );
                                                            }
                                                            Err(e) => {
                                                                tracing::warn!(
                                                                    "Heartbeat backup poll: Failed to get status for tunnel {}: {}",
                                                                    tunnel_id, e
                                                                );
                                                            }
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        _ = timeout_rx.recv() => {
                            // Heartbeat timeout - daemon appears offline
                            tracing::warn!("Daemon heartbeat timeout - reconnecting");

                            // Update daemon connection state in AppCore
                            super::event_handler::handle_daemon_connected(&state, false);

                            // Update status icon
                            status_icon.set_icon_name(Some("network-offline-symbolic"));
                            status_icon.set_tooltip_text(Some("✗ Connection timeout"));
                            status_icon.remove_css_class("daemon-connected");
                            status_icon.remove_css_class("daemon-connecting");
                            status_icon.add_css_class("daemon-offline");

                            // Refresh daemon page if we're currently viewing it
                            if *state.current_nav_page.borrow() == navigation::NavigationPage::Daemon {
                                if let Some(refresh) = state.daemon_page_refresh.borrow().as_ref() {
                                    tracing::debug!("Refreshing daemon page (daemon disconnected)");
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
                    tracing::info!("Reconnecting to daemon in {} seconds...", backoff.as_secs());
                }
                Err(e) => {
                    tracing::error!("Failed to start event listener: {}", e);
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
