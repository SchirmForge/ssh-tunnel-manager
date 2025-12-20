// SSH Tunnel Manager - System Tray Extension
// Provides system tray icon with quick access to tunnel management

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber;

mod tray;
mod daemon_monitor;
mod state;
mod notifications;
mod dialogs;

use state::TrayState;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Initialize GTK (needed for dialogs)
    gtk4::init().expect("Failed to initialize GTK");

    // Create shared state
    let state = Arc::new(RwLock::new(TrayState::new()?));

    // Start daemon monitor in background
    let monitor_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = daemon_monitor::start_monitor(monitor_state).await {
            eprintln!("Daemon monitor error: {}", e);
        }
    });

    // Create and run the tray icon
    tray::run_tray(state).await?;

    Ok(())
}
