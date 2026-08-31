// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Daemon communication module for GTK GUI
//!
//! All daemon communication code (DaemonClient, EventListener, TunnelEvent)
//! is now in gui-core for maximum code reuse across GUI front-ends.

// Re-export daemon types from gui-core
pub use ssh_tunnel_gui_core::{DaemonClient, EventListener, TunnelEvent};
