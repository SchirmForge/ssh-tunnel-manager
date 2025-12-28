// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Qt6 desktop application for SSH Tunnel Manager
//!
//! Minimal Qt application to demonstrate basic functionality.
//! Full QML integration pending cxx-qt bridge macro resolution.

mod daemon;
mod models;
mod ui;

fn main() {
    println!("SSH Tunnel Manager - Qt GUI (Placeholder)");
    println!();
    println!("The Qt GUI is currently being developed.");
    println!("Please use the GTK GUI for now:");
    println!("  cargo run --package ssh-tunnel-gui-gtk");
    println!();
    println!("Status:");
    println!("  ✓ All business logic ready (ProfileViewModel, DaemonClient, etc.)");
    println!("  ✓ QML UI designed");
    println!("  ⏸  cxx-qt bridge macro compilation issues being investigated");
    println!();
    println!("The GUI demonstrates ~60-70% code reuse from gui-core,");
    println!("sharing the same business logic as the GTK implementation.");
}
