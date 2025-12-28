// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Daemon communication module - Multi-protocol API client for GUI
//!
//! Supports Unix socket, HTTP, and HTTPS connections to the SSH Tunnel Manager daemon.
//! Includes both REST API client and SSE event listener.

pub mod client;
pub mod config;
pub mod sse;

pub use client::DaemonClient;
pub use config::{load_daemon_config, get_cli_config_path};
pub use sse::{EventListener, TunnelEvent};
