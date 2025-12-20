// System tray icon implementation

use anyhow::Result;
use ksni;
use ksni::menu::StandardItem;
use ssh_tunnel_common::{create_daemon_client, load_profile_by_id};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::state::{ConnectionStatus, TrayState};

/// Tray icon service
#[derive(Clone)]
struct TrayIcon {
    state: Arc<RwLock<TrayState>>,
}

impl TrayIcon {
    fn new(state: Arc<RwLock<TrayState>>) -> Self {
        Self { state }
    }

    /// Get icon name based on connection status
    fn get_icon_name(&self, status: ConnectionStatus) -> &'static str {
        match status {
            ConnectionStatus::Disconnected => "network-offline",
            ConnectionStatus::Connected => "network-idle",
            ConnectionStatus::Active => "network-transmit-receive",
        }
    }

    /// Launch the GUI application
    fn launch_gui(&self) {
        let _ = Command::new("ssh-tunnel-gui").spawn();
    }

    /// Start a tunnel (spawns async task)
    fn start_tunnel_async(&self, profile_id: uuid::Uuid) {
        let state = self.state.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::start_tunnel_impl(state, profile_id).await {
                eprintln!("Failed to start tunnel: {}", e);
            }
        });
    }

    /// Stop a tunnel (spawns async task)
    fn stop_tunnel_async(&self, profile_id: uuid::Uuid) {
        let state = self.state.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::stop_tunnel_impl(state, profile_id).await {
                eprintln!("Failed to stop tunnel: {}", e);
            }
        });
    }

    /// Start tunnel implementation
    async fn start_tunnel_impl(
        state: Arc<RwLock<TrayState>>,
        profile_id: uuid::Uuid,
    ) -> Result<()> {
        let config = {
            let state_lock = state.read().await;
            state_lock.daemon_config.clone()
        };

        // Add profile to recent list
        if let Ok(profile) = load_profile_by_id(&profile_id) {
            let mut state_lock = state.write().await;
            state_lock.add_recent_profile(&profile);
        }

        let client = create_daemon_client(&config)?;
        let url = format!("{}/api/tunnels/{}/start", config.daemon_base_url()?, profile_id);
        let request = client.post(&url);
        let request = ssh_tunnel_common::add_auth_header(request, &config)?;

        let response = request.send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to start tunnel: {}", response.status());
        }

        Ok(())
    }

    /// Stop tunnel implementation
    async fn stop_tunnel_impl(
        state: Arc<RwLock<TrayState>>,
        profile_id: uuid::Uuid,
    ) -> Result<()> {
        let config = {
            let state_lock = state.read().await;
            state_lock.daemon_config.clone()
        };

        let client = create_daemon_client(&config)?;
        let url = format!("{}/api/tunnels/{}/stop", config.daemon_base_url()?, profile_id);
        let request = client.post(&url);
        let request = ssh_tunnel_common::add_auth_header(request, &config)?;

        let response = request.send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to stop tunnel: {}", response.status());
        }

        Ok(())
    }
}

impl ksni::Tray for TrayIcon {
    fn icon_name(&self) -> String {
        // Use blocking_lock for sync context
        let status = self
            .state
            .blocking_read()
            .status;

        self.get_icon_name(status).to_string()
    }

    fn title(&self) -> String {
        let state = self.state.blocking_read();
        let count = state.active_tunnels.len();

        match state.status {
            ConnectionStatus::Disconnected => "SSH Tunnels (Disconnected)".to_string(),
            ConnectionStatus::Connected => "SSH Tunnels (Connected)".to_string(),
            ConnectionStatus::Active => format!("SSH Tunnels ({} active)", count),
        }
    }

    fn id(&self) -> String {
        "ssh-tunnel-manager".to_string()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    fn icon_theme_path(&self) -> String {
        "/usr/share/icons/hicolor".to_string()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let state = self.state.blocking_read();
        let status = state.status;
        let active_tunnels = state.active_tunnels.len();
        let recent_profile = state.get_recent_profile().cloned();
        drop(state); // Release the lock

        let mut menu = vec![];

        // Start menu item
        if let Some(recent) = recent_profile {
            let profile_name = recent.profile_name.clone();
            let profile_id = recent.profile_id;

            menu.push(ksni::MenuItem::Standard(StandardItem {
                label: format!("Start {}", profile_name),
                enabled: status != ConnectionStatus::Disconnected,
                activate: Box::new(move |this: &mut Self| {
                    this.start_tunnel_async(profile_id);
                }),
                ..Default::default()
            }));
        } else {
            // No recent profile
            menu.push(ksni::MenuItem::Standard(StandardItem {
                label: "Start Tunnel...".to_string(),
                enabled: status != ConnectionStatus::Disconnected,
                activate: Box::new(move |this: &mut Self| {
                    // Launch GUI to select profile
                    this.launch_gui();
                }),
                ..Default::default()
            }));
        }

        // Stop menu item
        if active_tunnels == 0 {
            menu.push(ksni::MenuItem::Standard(StandardItem {
                label: "Stop Tunnel".to_string(),
                enabled: false,
                ..Default::default()
            }));
        } else {
            // Get active tunnel info
            let state = self.state.blocking_read();
            let tunnels: Vec<_> = state.active_tunnels.values().cloned().collect();
            drop(state);

            if tunnels.len() == 1 {
                let tunnel = &tunnels[0];
                let profile_name = tunnel.profile_name.clone();
                let profile_id = tunnel.profile_id;

                menu.push(ksni::MenuItem::Standard(StandardItem {
                    label: format!("Stop {}", profile_name),
                    enabled: true,
                    activate: Box::new(move |this: &mut Self| {
                        this.stop_tunnel_async(profile_id);
                    }),
                    ..Default::default()
                }));
            } else {
                // Multiple tunnels - launch GUI to select
                menu.push(ksni::MenuItem::Standard(StandardItem {
                    label: "Stop Tunnel...".to_string(),
                    enabled: true,
                    activate: Box::new(move |this: &mut Self| {
                        this.launch_gui();
                    }),
                    ..Default::default()
                }));
            }
        }

        menu.push(ksni::MenuItem::Separator);

        // Settings (open GUI)
        menu.push(ksni::MenuItem::Standard(StandardItem {
            label: "Settings...".to_string(),
            activate: Box::new(|this: &mut Self| {
                this.launch_gui();
            }),
            ..Default::default()
        }));

        menu.push(ksni::MenuItem::Separator);

        // Quit
        menu.push(ksni::MenuItem::Standard(StandardItem {
            label: "Quit".to_string(),
            activate: Box::new(|_| {
                std::process::exit(0);
            }),
            ..Default::default()
        }));

        menu
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        // Single click - show status notification
        let state = self.state.blocking_read();
        let message = match state.status {
            ConnectionStatus::Disconnected => "Daemon not connected".to_string(),
            ConnectionStatus::Connected => "Connected to daemon, no active tunnels".to_string(),
            ConnectionStatus::Active => {
                let names: Vec<String> = state
                    .active_tunnels
                    .values()
                    .map(|t| t.profile_name.clone())
                    .collect();
                format!("Active tunnels:\n{}", names.join("\n"))
            }
        };
        drop(state);

        let _ = notify_rust::Notification::new()
            .summary("SSH Tunnel Manager")
            .body(&message)
            .timeout(notify_rust::Timeout::Milliseconds(3000))
            .show();
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        // Double click - open GUI
        self.launch_gui();
    }
}

/// Run the tray icon
pub async fn run_tray(state: Arc<RwLock<TrayState>>) -> Result<()> {
    let tray = TrayIcon::new(state.clone());

    let service = ksni::TrayService::new(tray);
    let handle = service.handle();

    // Spawn service in background thread (ksni needs its own thread)
    std::thread::spawn(move || {
        let _ = service.run();
    });

    // Periodically update the tray icon based on state changes
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Trigger icon update
        handle.update(|_tray: &mut TrayIcon| {
            // The tray will re-read state on next access
        });
    }
}
