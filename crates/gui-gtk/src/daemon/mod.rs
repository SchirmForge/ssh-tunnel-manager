// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// Daemon communication module - HTTP API integration for GUI

pub mod client;
pub mod sse;

pub use client::DaemonClient;
