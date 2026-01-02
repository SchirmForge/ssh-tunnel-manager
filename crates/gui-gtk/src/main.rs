// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// SSH Tunnel Manager - GUI Application
// GTK4 + libadwaita graphical interface for managing SSH tunnels

use gtk4::prelude::*;
use libadwaita as adw;

mod ui;
mod models;
mod utils;
mod daemon;

const APP_ID: &str = "com.github.ssh-tunnel-manager";

fn main() {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("ssh_tunnel_gtk=debug".parse().unwrap())
        )
        .init();

    // Initialize Tokio runtime for async operations
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    // Enter the runtime context so async operations work
    let _guard = runtime.enter();

    // Initialize GTK
    gtk4::init().expect("Failed to initialize GTK");

    // Initialize libadwaita
    adw::init().expect("Failed to initialize libadwaita");

    // Create application
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .build();

    // Connect activate signal to build UI
    app.connect_activate(|app| {
        ui::style::load();
        let window = ui::window::build(app);
        window.present();
    });

    // Run the application
    app.run();
}
